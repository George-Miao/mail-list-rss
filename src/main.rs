use std::net::SocketAddr;

use anyhow::{bail, Result};
use log::{debug, error, info, warn};
use mailin::{response, Handler, Response, SessionBuilder};
use mailparse::{parse_mail, MailHeaderMap};
use mongodb::{options::ClientOptions, Client, Collection};
use rss::{ChannelBuilder, Item, ItemBuilder};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};

mod db;
use db::*;

struct SmtpConnection {
    data: Option<Vec<u8>>,
    collection: Collection<Feed>,
}

impl SmtpConnection {
    pub fn from_collection(collection: Collection<Feed>) -> Self {
        Self {
            data: None,
            collection,
        }
    }
    pub fn end(&self) -> Result<()> {
        let data = self.data.as_ref().expect("data should be initialized");
        match parse_mail(data) {
            Ok(parsed) => {
                let feed: Feed = parsed.try_into()?;

                Ok(())
            }
            Err(e) => {
                warn!("Parse failed: {:?}", e);
                bail!("Parse failed")
            }
        }
    }
}

impl Handler for SmtpConnection {
    fn data_start(
        &mut self,
        _domain: &str,
        _from: &str,
        _is8bit: bool,
        _to: &[String],
    ) -> Response {
        self.data = Some(Vec::with_capacity(8 * 1024));
        response::OK
    }

    fn data(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.data
            .as_mut()
            .expect("data should be initialized")
            .extend(buf);
        Ok(())
    }

    fn data_end(&mut self) -> Response {
        self.end().unwrap_or_else(|e| warn!("{}", e));
        response::OK
    }
}

macro_rules! write_resp_to_writer {
    ($resp:ident, $write:ident) => {{
        let mut buf = Vec::with_capacity(1024);
        $resp.write_to(&mut buf)?;
        $write.write_all(&buf).await?;
        $write.flush().await?;
    }};
}

async fn handle(
    mut stream: TcpStream,
    addr: SocketAddr,
    collection: Collection<Feed>,
) -> Result<()> {
    info!("{} connected", addr);
    let (read, write) = stream.split();
    let mut lines = BufReader::new(read);
    let mut write = BufWriter::new(write);

    let handler = SmtpConnection::from_collection(collection);
    let mut session = SessionBuilder::new("test_server").build("127.0.0.1".parse()?, handler);
    let greeting = session.greeting();

    debug!("<- {:?}", greeting);

    write_resp_to_writer!(greeting, write);

    let mut buf = String::with_capacity(1024);

    while let Ok(num) = lines.read_line(&mut buf).await {
        if num == 0 {
            break;
        }

        debug!("-> {}", buf.replace("\r\n", ""));
        let resp = session.process(buf.as_bytes());
        debug!("<- {:?}", resp);

        write_resp_to_writer!(resp, write);

        buf.clear();
    }

    info!("{} disconnected", addr);
    info!("--------------------------------");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    simple_logger::init_with_level(log::Level::Debug)?;
    let listener = TcpListener::bind("127.0.0.1:10000").await?;
    let option = ClientOptions::parse("mongodb://localhost:27017").await?;
    let client = Client::with_options(option)?;
    let db = client.database("mail-list-rss");
    let collection = db.collection::<Feed>("feed");

    while let Ok((stream, addr)) = listener.accept().await {
        if let Err(e) = handle(stream, addr, collection.clone()).await {
            error!("{}", e)
        }
    }

    Ok(())
}
