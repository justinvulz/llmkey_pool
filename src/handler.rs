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
    pub proxy_api_key: String
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
    State(state): State<AppState>, 
    method: Method,
    uri: Uri, 
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
    let path_query = match query {
        "" => path.to_string(),
        _  => format!("{}&{}",path,query)
    };

    let target_url = uri::Builder::new()
        .scheme("https")
        .authority("generativelanguage.googleapis.com")
        .path_and_query(format!("/v1beta{}",path_query))
        .build()
        .map_err(|e| ProxyError::Internal(format!("URL build fail: {}", e)))?;



    let mut request_builder = state.http_client.request(method.clone(), target_url.to_string());

    for (name, value) in headers.iter() {
        let header_name = name.as_str();
        if !["host", "content-length", "accept-encoding", "connection", "user-agent", "authorization", "x-goog-api-key"]
            .contains(&header_name.to_lowercase().as_str())
        {
            request_builder = request_builder.header(name, value);
        }
    }

    request_builder = if path.starts_with("/openai") {
        request_builder.header("Authorization", format!("Bearer {}", api_key))
    } else {
        request_builder.header("x-goog-api-key", format!("{}", api_key))
    };

    
    if !body.is_empty() {
        request_builder = request_builder.body(body);
    }


    info!("Proxying request: {} {} to {}", method, uri, target_url);

    let (client,request) = request_builder.build_split();
    let request = request.map_err(|e| ProxyError::Upstream(format!("Failed to send request to Gemini API: {}", e)))?;


    let upstream_response = client.execute(request).await.map_err(|e| {
        ProxyError::Upstream(format!("Failed to send request to Gemini API: {}", e))
    })?;

    let mut response_builder = Response::builder().status(upstream_response.status());

    for (name, value) in upstream_response.headers().iter() {
        response_builder = response_builder.header(name, value);
    }

    let response_body = Body::from_stream(upstream_response.bytes_stream());

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
        None => headers.get("x-goog-api-key").and_then(|value| value.to_str().ok()),
    };

    match provided_token {
        Some(token) if token == state.proxy_api_key => {
            Ok(next.run(request).await)
        }
        _ => {
            error!("Auth error");
            Err(ProxyError::Unauthorized(
                "Invalid or missing 'Authorization: Bearer <TOKEN>' header".to_string(),
            ))
        },

    }
}

