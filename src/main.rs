use anyhow::Result;
use crossfire::mpsc::{bounded_tx_blocking_rx_future, RxFuture, SharedSenderBRecvF, TxBlocking};
use mongodb::{options::ClientOptions, Client};

mod db;
mod smtp;
mod web;

use db::*;
use smtp::*;
use web::*;

type TX = TxBlocking<Feed, SharedSenderBRecvF>;
type RX = RxFuture<Feed, SharedSenderBRecvF>;

#[tokio::main]
async fn main() -> Result<()> {
    simple_logger::init_with_level(log::Level::Info)?;

    let client = Client::with_options(ClientOptions::parse("mongodb://localhost:27017").await?)?;
    let db = client.database("mail-list-rss");
    let feeds = db.collection::<Feed>("feed");

    let (tx, rx) = bounded_tx_blocking_rx_future::<Feed>(10);
    let bg = tokio::spawn(database_servo(feeds.clone(), rx));
    let server = tokio::spawn(web_server(feeds));

    smtp_server(tx).await?;

    bg.abort();
    server.abort();

    Ok(())
}
