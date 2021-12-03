use std::fmt::Display;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use log::{info, warn};
use mailparse::{MailHeaderMap, ParsedMail};
use mongodb::Collection;
use rss::{GuidBuilder, Item, ItemBuilder};
use serde::{Deserialize, Serialize};

use crate::RX;

pub type Feeds = Collection<Feed>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Feed {
    pub created_at: DateTime<Utc>,
    pub title: String,
    pub author: String,
    pub content: String,
    pub id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Index {
    pub id: String,
}

impl Display for Feed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Feed #{} [{}] <{}> by {} (len {})",
            self.id,
            self.created_at.format("%T"),
            self.title,
            self.author,
            self.content.len()
        )
    }
}

impl<'a> TryFrom<ParsedMail<'a>> for Feed {
    type Error = anyhow::Error;
    fn try_from(val: ParsedMail<'a>) -> Result<Self> {
        let author = val
            .headers
            .get_first_value("From")
            .ok_or_else(|| anyhow!("No From"))?;
        let title = val
            .headers
            .get_first_value("Subject")
            .unwrap_or_else(|| author.to_owned());
        let created_at = Utc::now();
        let content = val.get_body()?;

        Ok(Feed {
            created_at,
            title,
            author,
            content,
            id: nanoid::nanoid!(10),
        })
    }
}

impl From<Feed> for Item {
    fn from(feed: Feed) -> Self {
        ItemBuilder::default()
            .title(feed.title)
            .link(Some(format!("https://rss.miao.do/feeds/{}", feed.id)))
            .author(Some(feed.author))
            .pub_date(Some(feed.created_at.to_rfc2822()))
            .guid(Some(
                GuidBuilder::default()
                    .permalink(true)
                    .value(format!("https://rss.miao.do/feeds/{}", feed.id))
                    .build(),
            ))
            .content(Some(feed.content))
            .link(Some("https://baidu.com".to_owned()))
            .build()
    }
}

pub async fn database_servo(collection: Feeds, rx: RX) {
    info!("Database servo starting");

    while let Ok(feed) = rx.recv().await {
        info!("{}", feed);
        if let Err(e) = collection.insert_one(feed, None).await {
            warn!("Error insert doc: {}", e)
        }
    }

    info!("Database servo stopping");
}

#[derive(Deserialize, Serialize)]
pub struct Summary {
    pub title: String,
    pub id: String,
}
#[derive(Deserialize, Serialize)]
pub struct List {
    pub items: Vec<Summary>,
}
