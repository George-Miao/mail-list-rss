#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::all)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::redundant_pub_crate)]

use std::time::Duration;

use anyhow::Result;
use mongodb::{options::ClientOptions, Client};
use tokio::{
    signal::ctrl_c,
    spawn,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};
use tracing::{info, metadata::LevelFilter, warn};
use tracing_subscriber::util::SubscriberInitExt;

mod_use::mod_use![config, db, smtp, web];

type TX = UnboundedSender<Feed>;
type RX = UnboundedReceiver<Feed>;

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

    let (tx, rx) = unbounded_channel::<Feed>();

    tokio::select! {
        _ = spawn(db::servo(feeds.clone(), rx)) => {
            warn!("SMTP server stopped");
        },
        _ = spawn(web::server(feeds)) => {
            warn!("SMTP server stopped");
        },
        _ = spawn(smtp::server(tx)) => {
            warn!("SMTP server stopped");
        }
        _ = ctrl_c() => {
            info!("Ctrl-C received, shutting down");
        }
    }

    Ok(())
}
