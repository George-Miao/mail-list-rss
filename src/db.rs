use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use mailparse::{MailHeaderMap, ParsedMail};
use rss::{Enclosure, Item, ItemBuilder};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Feed {
    created_at: DateTime<Utc>,
    title: String,
    author: String,
    content: String,
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
        })
    }
}

impl From<Feed> for Item {
    fn from(feed: Feed) -> Self {
        ItemBuilder::default()
            .title(feed.title)
            .author(Some(feed.author))
            .content(Some(feed.content))
            .pub_date(Some(feed.created_at.to_string()))
            .build()
    }
}
