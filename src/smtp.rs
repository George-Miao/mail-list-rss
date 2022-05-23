use std::net::SocketAddr;

use anyhow::{bail, Result};
use mail_parser::Message;
use mailin::{response, Handler, Response, SessionBuilder};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, TcpStream},
};
use tracing::{debug, error, info, warn};

use crate::{db::Feed, Config, TX};

struct SmtpConnection {
    data: Option<Vec<u8>>,
    tx: TX,
}

impl SmtpConnection {
    pub const fn new(tx: TX) -> Self {
        Self { data: None, tx }
    }

    pub fn end(&mut self) -> Result<()> {
        let data = self.data.take().expect("data should be initialized");
        match Message::parse(&data) {
            Some(parsed) => {
                let feed: Feed = (data.as_slice(), parsed).try_into()?;
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
        let domain = &Config::get().domain;
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

async fn handle(mut stream: TcpStream, addr: SocketAddr, tx: TX) -> Result<()> {
    debug!(target: "SMTP", "SMTP: {} connected", addr);
    let (read, write) = stream.split();

    let mut lines = BufReader::new(read);
    let mut write = Box::pin(BufWriter::new(write));

    let handler = SmtpConnection::new(tx);
    let mut session = SessionBuilder::new("mail-list-rss-server").build(addr.ip(), handler);
    let greeting = session.greeting();

    debug!(target: "SMTP", "   >>> OUT: {:?}", greeting);

    greeting.write_to_async(&mut write).await?;
    write.flush().await?;

    let mut buf = String::with_capacity(1024);

    while let Ok(num) = lines.read_line(&mut buf).await {
        if num == 0 {
            break;
        }

        debug!(target: "SMTP", "   >>> IN:  {}", buf.replace("\r\n", ""));
        let resp = session.process(buf.as_bytes());
        debug!(target: "SMTP", "   >>> OUT: {:?}", resp);
        resp.write_to_async(&mut write).await?;
        write.flush().await?;

        buf.clear();
    }

    debug!(target: "SMTP", "SMTP: {} disconnected", addr);
    Ok(())
}

pub async fn server(tx: TX) -> Result<()> {
    info!(target: "SMTP", "Starting");
    let config = Config::get();
    while let Ok((stream, addr)) = TcpListener::bind(format!("0.0.0.0:{}", config.smtp_port))
        .await?
        .accept()
        .await
    {
        let tx = tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(stream, addr, tx).await {
                error!("{}", e);
            }
        });
    }
    info!(target: "SMTP", "Stopping");
    Ok(())
}
