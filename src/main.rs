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
    // -----------------------------------------------------------------
    // 從 config.toml 載入設定
    // -----------------------------------------------------------------
    let config_path = "config.toml";
    let config_content = std::fs::read_to_string(config_path)
        .unwrap_or_else(|e| panic!("無法讀取設定檔 {}: {}", config_path, e));
    let config: Config = toml::from_str(&config_content)
        .unwrap_or_else(|e| panic!("無法解析設定檔 {}: {}", config_path, e));

    info!("已載入設定: {:?}", config); // 輸出載入的設定，方便除錯

    // -----------------------------------------------------------------
    // 獲取 Gemini API 密鑰 (直接從 config.toml 讀取)
    // -----------------------------------------------------------------
    let gemini_api_keys = config.proxy.gemini_api_keys;

    if gemini_api_keys.is_empty() {
        panic!("設定檔中沒有找到有效的 Gemini API 密鑰 (proxy.gemini_api_keys)。請檢查設定內容。");
    }
    info!("從設定檔中載入 {} 個 Gemini API 密鑰。", gemini_api_keys.len());


    // -----------------------------------------------------------------
    // 獲取代理服務器自身的 API 密鑰
    // -----------------------------------------------------------------
    let proxy_api_key = config.proxy.proxy_api_key.filter(|s| !s.is_empty()); // 如果為空字串則當作未設定

    if proxy_api_key.is_some() {
        info!("代理伺服器認證已啟用。");
    } else {
        warn!("代理伺服器認證已停用。在生產環境中請考慮設定 proxy_api_key。");
    }

    // 創建應用程式狀態
    let app_state = AppState {
        apikeys: Arc::new(Mutex::new(KeyPool::new(gemini_api_keys))),
        http_client: Arc::new(Client::new()),
        proxy_api_key,
    };

    // 路由設定
    let app = Router::new()
        // 健康檢查路由不需要認證
        .route("/healthz", any(|| async {"ok"}))
        // 其他所有路由都通過認證中間件
        .fallback(any(proxy_handler))
        .layer(axum::middleware::from_fn_with_state(app_state.clone(), auth_middleware)) // 應用認證中間件
        .with_state(app_state); // 將狀態注入到所有路由中

    // 啟動伺服器
    let addr_str = format!("{}:{}", config.server.host, config.server.port);
    let addr: SocketAddr = addr_str.parse()
        .unwrap_or_else(|e| panic!("無效的伺服器地址或端口 '{}': {}", addr_str, e));
    info!("Gemini 代理伺服器已啟動於 http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener,app).await.unwrap();
}
