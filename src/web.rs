use std::{net::SocketAddr, str::FromStr};

use anyhow::Result;
use axum::{
    extract::{Extension, Path},
    handler::Handler,
    http::{
        header::{self, HeaderName, CONTENT_TYPE},
        uri::{Authority, Scheme},
        HeaderValue, Request, StatusCode,
    },
    response::{Headers, Html, IntoResponse, Redirect, Response},
    routing::{any, get},
    AddExtensionLayer, Json, Router,
};
use axum_extra::middleware::{middleware_fn, Next};
use chrono::Utc;
use futures::{
    future::{ready, Ready},
    StreamExt, TryStreamExt,
};
use mongodb::{bson::doc, options::FindOptions};
use serde::{Deserialize, Serialize};
use tower_http::{
    auth::RequireAuthorizationLayer,
    cors,
    set_header::SetResponseHeaderLayer,
    trace::{OnRequest, OnResponse, TraceLayer},
};
use tracing::{info, warn};

use crate::{
    db::{Feeds, List, Summary},
    Config,
};

fn utf8_header(res: &Response) -> Option<HeaderValue> {
    if let Some(header) = res.headers().get(CONTENT_TYPE) {
        if let Ok(header) = header.to_str() {
            if !header.ends_with("charset=utf-8") {
                let mut header = header.to_owned();
                header.push_str("; charset=utf-8");
                return HeaderValue::from_str(&header).ok();
            }
        }
    }
    None
}

async fn http_rediretor<B: Send + Sync>(req: Request<B>, next: Next<B>) -> impl IntoResponse {
    let config = Config::get();

    match req
        .headers()
        .get(HeaderName::from_lowercase(b"x-forwarded-proto").unwrap())
    {
        Some(schema) if schema.to_str().map(|x| x != "https").unwrap_or(true) => {
            let mut parts = req.uri().clone().into_parts();
            parts.scheme = Some(Scheme::HTTPS);
            parts.authority = Some(Authority::from_str(&config.domain).expect("Bad domain"));
            Err(Redirect::permanent(parts.try_into().unwrap()))
        }
        _ => Ok(next.run(req).await),
    }
}

#[derive(Copy, Clone)]
struct Logger {}

impl<B> OnRequest<B> for Logger {
    fn on_request(&mut self, request: &axum::http::Request<B>, _: &tracing::Span) {
        let method = request.method();
        let route = request.uri().path();

        info!(target: "web",  "=> {} {}", method, route);
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
        info!(target: "web", status, ?latency, "<=");
    }
}

trait RouterExt {
    fn auth_layer(self, username: Option<&str>, password: Option<&str>) -> Self;
}

impl RouterExt for Router {
    fn auth_layer(self, username: Option<&str>, password: Option<&str>) -> Self {
        if username.is_some() && password.is_some() {
            info!(
                target: "web",
                "Using basic auth"
            );
            self.route_layer(RequireAuthorizationLayer::basic(
                username.as_ref().unwrap(),
                password.as_ref().unwrap(),
            ))
        } else {
            warn!(target: "web", "No auth configured, this can be dangerous and should only be used in development");
            self
        }
    }
}

pub async fn server(collection: Feeds) -> Result<()> {
    let logger = Logger {};

    let utf8_layer = SetResponseHeaderLayer::overriding(CONTENT_TYPE, utf8_header);
    let config = Config::get();

    let app = Router::new()
        .route("/", get(index))
        .route("/feeds/:key", get(raw))
        .route("/feeds", get(list.layer(utf8_layer)))
        .route("/rss", get(rss))
        .layer(AddExtensionLayer::new(collection))
        .layer(
            TraceLayer::new_for_http()
                .on_request(logger)
                .on_response(logger),
        )
        .auth_layer(config.username.as_deref(), config.password.as_deref())
        .route("/health", any(|| async { "OK" }))
        .route_layer(middleware_fn::from_fn(http_rediretor))
        .route_layer(
            cors::CorsLayer::new()
                .allow_headers(cors::any())
                .allow_methods(cors::any())
                .allow_origin(cors::any()),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], config.web_port));

    info!(target: "web", "Starting");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    info!(target: "web", "Stopped");

    Ok(())
}

fn index() -> Ready<impl IntoResponse> {
    ready(Html(include_str!("../front/dist/index.html")))
}

async fn rss(Extension(feed): Extension<Feeds>) -> impl IntoResponse {
    match render_feeds(feed).await {
        Ok(content) => (
            StatusCode::OK,
            Headers(vec![(
                header::CONTENT_TYPE,
                "application/xml; charset=utf-8",
            )]),
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
    let config = Config::get();
    let option = FindOptions::builder()
        .limit(i64::from(config.per_page))
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
        .pub_date(Some(Utc::now().to_rfc2822()))
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

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Key {
    pub key: String,
}

async fn raw(Path(map): Path<Key>, Extension(feeds): Extension<Feeds>) -> impl IntoResponse {
    let key = &map.key;
    match feeds.find_one(doc! { "id" : key }, None).await {
        Ok(Some(res)) => (
            StatusCode::OK,
            Headers(vec![(header::CONTENT_TYPE, "text/html; charset=utf-8")]),
            res.content,
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Headers(vec![]),
            format!("Cannot find {}", key),
        ),
        Err(error) => {
            warn!(target: "web", %error, "Database error");

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Headers(vec![]),
                error.to_string(),
            )
        }
    }
}
