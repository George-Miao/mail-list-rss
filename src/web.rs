use std::{collections::HashMap, net::SocketAddr};

use anyhow::Result;
use axum::{
    extract::{Extension, Path},
    http::{header, StatusCode},
    response::{Headers, IntoResponse},
    routing::{any, get},
    AddExtensionLayer, Json, Router,
};
use chrono::Utc;
use futures::{StreamExt, TryStreamExt};
use log::info;
use mongodb::{bson::doc, options::FindOptions};

use crate::{
    config::get_config,
    db::{Feeds, List, Summary},
};

pub async fn web_server(collection: Feeds) -> Result<()> {
    let app = Router::new()
        .route("/feeds/:key", get(raw))
        .route("/rss", get(rss))
        .route("/list", get(list))
        .layer(AddExtensionLayer::new(collection))
        .route("/health", any(|| async { "OK" }));

    let addr = SocketAddr::from(([0, 0, 0, 0], get_config().web_port));

    info!("HTTP server starting");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    info!("HTTP server stopped");

    Ok(())
}

async fn rss(Extension(feed): Extension<Feeds>) -> impl IntoResponse {
    match render_feeds(feed).await {
        Ok(content) => (
            StatusCode::OK,
            Headers(vec![(header::CONTENT_TYPE, "application/xml")]),
            content,
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Headers(vec![]),
            e.to_string(),
        ),
    }
}

async fn render_feeds(feeds: Feeds) -> Result<String> {
    let config = get_config();
    let option = FindOptions::builder()
        .limit(config.per_page as i64)
        .sort(None)
        .build();
    let feeds = feeds
        .find(None, option)
        .await?
        .try_fold(Vec::with_capacity(10), |mut acc, x| async move {
            acc.push(x.into_rss());
            Ok(acc)
        })
        .await?;

    let ret = rss::ChannelBuilder::default()
        .title("Mail List")
        .generator(Some("http://github.com/George-Miao/mail-list-rss".into()))
        .link("http://github.com/George-Miao/mail-list-rss")
        .pub_date(Utc::now().to_rfc2822())
        .items(feeds)
        .build()
        .to_string();
    Ok(ret)
}

async fn list(Extension(feeds): Extension<Feeds>) -> impl IntoResponse {
    Json(render_list(feeds).await.unwrap())
}

async fn render_list(feeds: Feeds) -> Result<List> {
    let res = feeds
        .find(None, None)
        .await?
        .filter_map(|x| async move {
            x.ok().map(|x| Summary {
                title: x.title,
                id: x.id,
            })
        })
        .collect::<Vec<_>>()
        .await;

    Ok(List { items: res })
}

async fn raw(
    Path(map): Path<HashMap<String, String>>,
    Extension(feeds): Extension<Feeds>,
) -> impl IntoResponse {
    let key = map.get("key").expect("key should exist");
    info!("Key: {}", key);
    let res = feeds.find_one(doc! { "id" : key }, None).await;
    match res {
        Ok(Some(res)) => (
            StatusCode::OK,
            Headers(vec![(header::CONTENT_TYPE, "text/html")]),
            res.content,
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Headers(vec![]),
            format!("Cannot find {}", key),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Headers(vec![]),
            e.to_string(),
        ),
    }
}
