#![feature(once_cell)]

use anyhow::Result;
use crossfire::mpsc::{bounded_tx_blocking_rx_future, RxFuture, SharedSenderBRecvF, TxBlocking};
use mongodb::{options::ClientOptions, Client};

mod config;
mod db;
mod smtp;
mod web;

use config::*;
use db::*;
use smtp::*;
use web::*;

type TX = TxBlocking<Feed, SharedSenderBRecvF>;
type RX = RxFuture<Feed, SharedSenderBRecvF>;

#[tokio::main]
async fn main() -> Result<()> {
    simple_logger::init_with_level(log::Level::Info)?;

    let config = get_config();

    let client = Client::with_options(ClientOptions::parse(&config.mongo_con_str).await?)?;
    let db = client.database(&config.mongo_db_name);
    let feeds = db.collection::<Feed>("feed");

    let (tx, rx) = bounded_tx_blocking_rx_future::<Feed>(10);

    let bg = tokio::spawn(database_servo(feeds.clone(), rx));
    let server = tokio::spawn(web_server(feeds));

    smtp_server(tx).await?;

    bg.abort();
    server.abort();

    Ok(())
}
