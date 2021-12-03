use std::net::SocketAddr;

use anyhow::{bail, Result};
use log::{debug, error, info, warn};
use mailin::{response, Handler, Response, SessionBuilder};
use mailparse::parse_mail;
use tokio::net::TcpStream;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpListener,
};

use crate::{db::Feed, TX};

struct SmtpConnection {
    data: Option<Vec<u8>>,
    tx: TX,
}

impl SmtpConnection {
    pub fn new(tx: TX) -> Self {
        Self { data: None, tx }
    }
    pub fn end(&self) -> Result<()> {
        let data = self.data.as_ref().expect("data should be initialized");
        info!("{}", String::from_utf8(data.to_owned())?);
        match parse_mail(data) {
            Ok(parsed) => {
                let feed: Feed = parsed.try_into()?;
                self.tx.send(feed)?;
                Ok(())
            }
            Err(e) => {
                bail!("Parse failed: {:?}", e)
            }
        }
    }
}

impl Handler for SmtpConnection {
    fn data_start(&mut self, _: &str, _: &str, _: bool, _: &[String]) -> Response {
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

async fn handle(mut stream: TcpStream, addr: SocketAddr, tx: TX) -> Result<()> {
    info!("{} connected", addr);
    let (read, write) = stream.split();
    let mut lines = BufReader::new(read);
    let mut write = BufWriter::new(write);

    let handler = SmtpConnection::new(tx);
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

pub async fn smtp_server(tx: TX) -> Result<()> {
    info!("SMTP server starting");
    while let Ok((stream, addr)) = TcpListener::bind("127.0.0.1:10000").await?.accept().await {
        if let Err(e) = handle(stream, addr, tx.clone()).await {
            error!("{}", e)
        }
    }
    info!("SMTP server stopping");
    Ok(())
}
