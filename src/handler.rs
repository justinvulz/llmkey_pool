use axum::{
    body::{Body, Bytes},
    extract::{FromRef, Request, State},
    http::{self, uri, HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    RequestExt
};
use tracing::{info,error};
use reqwest::Client;
use std::{
    sync::{Arc, Mutex}
    
};
use crate::key::KeyPool;

#[derive(Clone)]
pub struct AppState {
    pub apikeys: Arc<Mutex<KeyPool>>,    
    pub http_client: Arc<Client>,
    pub proxy_api_key: Option<String>
}


pub enum ProxyError {
    Internal(String),
    Upstream(String),
    Unauthorized(String), // 未授權錯誤
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ProxyError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ProxyError::Upstream(msg) => (StatusCode::BAD_GATEWAY, msg),
            ProxyError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg), // 401 狀態碼
        };

        // error!("Proxy Error (Status: {}): {}", status, error_message);
        (status, format!("Proxy Error: {}", error_message)).into_response()
    }
}

pub async fn proxy_handler(
    State(state): State<AppState>, // 注入應用程式狀態
    method: Method,
    uri: Uri, // 使用 Uri 獲取完整的路徑和查詢參數
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {

    let api_key = {
        let mut manager = state.apikeys.lock().map_err(|e| {
            ProxyError::Internal(format!("Failed to lock API key manager mutex: {}", e))
        })?;
        manager.get_key()
    };

    let path = uri.path();
    let query = uri.query().unwrap_or("");
    //https://generativelanguage.googleapis.com/v1beta/openai/
    let target_url = uri::Builder::new()
        .scheme("https")
        .authority("generativelanguage.googleapis.com")
        .path_and_query(format!("/v1beta/openai{}{}",path, query))
        .build()
        .map_err(|e| ProxyError::Internal(format!("URL build fail: {}", e)))?;



    let mut request_builder = state.http_client.request(method.clone(), target_url.to_string());

    // 複製所有原始請求頭，但排除幾個由 reqwest 或 HTTP 協議本身處理的頭
    // 並排除我們自己的認證頭 (Authorization)
    for (name, value) in headers.iter() {
        let header_name = name.as_str();
        if !["host", "content-length", "accept-encoding", "connection", "user-agent", "authorization"]
            .contains(&header_name.to_lowercase().as_str())
        {
            request_builder = request_builder.header(name, value);
        }
    }
    request_builder = request_builder.header("Authorization", format!("Bearer {}", api_key));

    // 如果有請求體，則添加
    if !body.is_empty() {
        request_builder = request_builder.body(body);
    }

    info!(
            "Proxying request: {} {} to {}",
            method, uri, target_url
        );
    // 發送請求並獲取響應
    let upstream_response = request_builder.send().await.map_err(|e| {
        ProxyError::Upstream(format!("Failed to send request to Gemini API: {}", e))
    })?;

    // 構建要發送回客戶端的響應
    let mut response_builder = Response::builder().status(upstream_response.status());

    // 複製上游響應的所有頭
    for (name, value) in upstream_response.headers().iter() {
        response_builder = response_builder.header(name, value);
    }

    // 將上游響應的字節流直接轉發給下游客戶端
    let response_body = Body::from_stream(upstream_response.bytes_stream());

    // 構建最終響應
    let response = response_builder.body(response_body).map_err(|e| {
        ProxyError::Internal(format!("Failed to build response: {}", e))
    })?;

    Ok(response)
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request, 
    next: axum::middleware::Next,
) -> Result<Response, ProxyError> {

    if let Some(expected_key) = &state.proxy_api_key {

        let auth_header = headers
            .get(http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());

        let provided_token = match auth_header {
            Some(header_value) => {
                if let Some(token_str) = header_value.strip_prefix("Bearer ").map(|s| s.trim()) {
                    Some(token_str)
                } else {
                    None 
                }
            }
            None => None,
        };

        match provided_token {
            Some(token) if token == expected_key => {
                Ok(next.run(request).await)
            }
            _ => {
                error!("Auth error");
                Err(ProxyError::Unauthorized(
                    "Invalid or missing 'Authorization: Bearer <TOKEN>' header".to_string(),
                ))
        },

        }
    } else {
        Ok(next.run(request).await)
    }
}

