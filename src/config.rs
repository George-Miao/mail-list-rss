use std::{env::var, lazy::SyncLazy};

use anyhow::Result;

static CONFIG: SyncLazy<Config> = SyncLazy::new(|| Config::from_env().unwrap());

#[derive(Clone, Debug)]
pub struct Config {
    pub web_port: u16,
    pub smtp_port: u16,
    pub per_page: u16,
    pub domain: String,
    pub mongo_con_str: String,
    pub mongo_db_name: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let ret = Self {
            web_port: var("WEB_PORT").map_or_else(|_| Ok(8080), |x| x.parse())?,
            smtp_port: var("SMTP_PORT").map_or_else(|_| Ok(10000), |x| x.parse())?,
            per_page: var("PER_PAGE").map_or_else(|_| Ok(10), |x| x.parse())?,
            domain: var("DOMAIN").unwrap_or_else(|_| "example.com".to_owned()),
            mongo_con_str: var("MONGO_CON_STR")
                .unwrap_or_else(|_| "mongodb://localhost:27017".to_owned()),
            mongo_db_name: var("MONGO_DB_NAME").unwrap_or_else(|_| "mail-list-rss".to_owned()),
        };
        Ok(ret)
    }
}

#[inline]
pub fn get_config<'a>() -> &'a Config {
    &CONFIG
}
