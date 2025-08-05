use axum::{
    // body::{Body, Bytes},
    // extract::{FromRef, Request, State},
    // http::{HeaderMap, Method, StatusCode, Uri},
    // response::{IntoResponse, Response},
    routing::any,
    Router,
};

use reqwest::Client;
use std::{
    // fs::File,
    // io::{self, BufReader, BufRead},
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use llmkey_pool::{config::Config, key::KeyPool};
use llmkey_pool::handler::{AppState, proxy_handler, auth_middleware};
use tracing::{info, warn, error};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use toml;
// use serde::Deserialize;

#[tokio::main]
async fn main() {

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let config_path = "config.toml";
    let config_content = std::fs::read_to_string(config_path)
        .unwrap_or_else(|e| panic!("{}: {}", config_path, e));
    let config: Config = toml::from_str(&config_content)
        .unwrap_or_else(|e| panic!("Can't parse config {}: {}", config_path, e));

    info!("Loaded config : {:?}", config); 

    let gemini_api_keys = config.proxy.gemini_api_keys;

    if gemini_api_keys.is_empty() {
        panic!("No any Gemini API key found.");
    }
    info!("Found {} Gemini API keys.", gemini_api_keys.len());


    let proxy_api_key = config.proxy.proxy_api_key; 


    let app_state = AppState {
        apikeys: Arc::new(Mutex::new(KeyPool::new(gemini_api_keys))),
        http_client: Arc::new(Client::new()),
        proxy_api_key,
    };

    let app = Router::new()
        .route("/healthz", any(|| async {"ok"}))
        .fallback(any(proxy_handler))
        .layer(axum::middleware::from_fn_with_state(app_state.clone(), auth_middleware))  
        .with_state(app_state);  

    // 啟動伺服器
    let addr_str = format!("{}:{}", config.server.host, config.server.port);
    let addr: SocketAddr = addr_str.parse()
        .unwrap_or_else(|e| panic!("無效的伺服器地址或端口 '{}': {}", addr_str, e));
    info!("Gemini 代理伺服器已啟動於 http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener,app).await.unwrap();
}
