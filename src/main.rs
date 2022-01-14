use std::{time::Duration};

use anyhow::Result;
use crossfire::mpsc::{bounded_tx_blocking_rx_future, RxFuture, SharedSenderBRecvF, TxBlocking};
use mongodb::{options::ClientOptions, Client};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

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
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    let config = get_config();

    let mongo_client = {
        let mut opt = ClientOptions::parse(&config.mongo_con_str).await?;
        opt.connect_timeout = Some(Duration::from_secs(1));
        Client::with_options(opt)?
    };

    let db_names = mongo_client
        .list_database_names(None, None)
        .await?
        .join(" / ");

    info!(db = db_names.as_str(), "Databases");

    let db = mongo_client.database(&config.mongo_db_name);
    let feeds = db.collection::<Feed>("feed");

    let (tx, rx) = bounded_tx_blocking_rx_future::<Feed>(10);

    let bg = tokio::spawn(database_servo(feeds.clone(), rx));
    let server = tokio::spawn(web_server(feeds));

    smtp_server(tx).await?;

    bg.abort();
    server.abort();

    Ok(())
}
