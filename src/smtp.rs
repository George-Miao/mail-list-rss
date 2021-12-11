use std::net::SocketAddr;

use anyhow::{bail, Result};
use log::{debug, error, info, warn};
use mail_parser::Message;
use mailin::{response, Handler, Response, SessionBuilder};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, TcpStream},
};

use crate::{config::get_config, db::Feed, TX};

struct SmtpConnection {
    data: Option<Vec<u8>>,
    tx: TX,
}

impl SmtpConnection {
    pub fn new(tx: TX) -> Self {
        Self { data: None, tx }
    }
    pub fn end(&self) -> Result<()> {
        let data = self.data.to_owned().expect("data should be initialized");
        match Message::parse(&data) {
            Some(parsed) => {
                let feed: Feed = (&data, parsed).try_into()?;
                self.tx.send(feed)?;
                Ok(())
            }
            None => {
                bail!("Parse failed")
            }
        }
    }
}

impl Handler for SmtpConnection {
    fn rcpt(&mut self, to: &str) -> Response {
        let domain = &get_config().domain;
        //  Block any rcpt that's not on my domain
        if to.contains(domain) {
            response::OK
        } else {
            response::NO_SERVICE
        }
    }
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
    let mut session = SessionBuilder::new("mail-list-rss-server").build(addr.ip(), handler);
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
    let config = get_config();
    while let Ok((stream, addr)) = TcpListener::bind(format!("0.0.0.0:{}", config.smtp_port))
        .await?
        .accept()
        .await
    {
        if let Err(e) = handle(stream, addr, tx.clone()).await {
            error!("{}", e)
        }
    }
    info!("SMTP server stopping");
    Ok(())
}
