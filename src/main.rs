#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::all)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]

use std::time::Duration;

use anyhow::Result;
use crossfire::mpsc::{bounded_tx_blocking_rx_future, RxFuture, SharedSenderBRecvF, TxBlocking};
use mongodb::{options::ClientOptions, Client};
use tracing::{info, metadata::LevelFilter};
use tracing_subscriber::util::SubscriberInitExt;

mod_use::mod_use![config, db, smtp, web];

type TX = TxBlocking<Feed, SharedSenderBRecvF>;
type RX = RxFuture<Feed, SharedSenderBRecvF>;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .without_time()
        .compact()
        .with_max_level(LevelFilter::INFO)
        .finish()
        .init();

    let config = Config::get();

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

    let database_servo = tokio::spawn(servo(feeds.clone(), rx));
    let web_server = tokio::spawn(web::server(feeds));

    smtp::server(tx).await?;

    database_servo.abort();
    web_server.abort();

    Ok(())
}
