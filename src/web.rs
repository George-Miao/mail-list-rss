use std::{collections::HashMap, net::SocketAddr};

use anyhow::Result;
use axum::{
    extract::{Extension, Path},
    http::{header, StatusCode},
    response::{Headers, Html, IntoResponse},
    routing::{any, get},
    AddExtensionLayer, Json, Router,
};
use chrono::Utc;
use futures::{StreamExt, TryStreamExt};
use mongodb::{bson::doc, options::FindOptions};
use tower_http::trace::{OnRequest, OnResponse, TraceLayer};
use tracing::{info, Level};

use crate::{
    config::get_config,
    db::{Feeds, List, Summary},
};

#[derive(Copy, Clone)]
struct Logger {}

impl<B> OnRequest<B> for Logger {
    fn on_request(&mut self, request: &axum::http::Request<B>, _: &tracing::Span) {
        let route = request.uri().path();
        tracing::event!(target: "web", Level::INFO, route);
    }
}

impl<B> OnResponse<B> for Logger {
    fn on_response(
        self,
        response: &axum::http::Response<B>,
        latency: std::time::Duration,
        _: &tracing::Span,
    ) {
        let status = response.status().as_u16();
        let time = latency.as_secs_f32();
        tracing::event!(target: "web", Level::INFO, status, time)
    }
}

pub async fn web_server(collection: Feeds) -> Result<()> {
    let logger = Logger {};
    let app = Router::new()
        .route("/", get(index))
        .route("/feeds/:key", get(raw))
        .route("/feeds", get(list))
        .route("/rss", get(rss))
        .layer(AddExtensionLayer::new(collection))
        .route("/health", any(|| async { "OK" }))
        .layer(
            TraceLayer::new_for_http()
                .on_request(logger)
                .on_response(logger),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], get_config().web_port));

    info!(target: "web", "Starting");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    info!(target: "web", "Stopped");

    Ok(())
}

async fn index() -> impl IntoResponse {
    Html(include_str!("./static/index.html"))
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
        .sort(doc! { "created_at": -1 })
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
        .find(
            None,
            FindOptions::builder()
                .sort(doc! { "created_at": -1 })
                .build(),
        )
        .await?
        .filter_map(|x| async move {
            x.ok().map(|x| Summary {
                create_at: x.created_at.to_rfc2822(),
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
