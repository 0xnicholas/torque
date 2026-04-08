use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

const API_KEY_HEADER: &str = "x-api-key";

pub async fn auth_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let api_key = request
        .headers()
        .get(API_KEY_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    match api_key {
        Some(key) if !key.is_empty() => {
            let mut request = request;
            request.extensions_mut().insert(key);
            Ok(next.run(request).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

pub fn extract_api_key(request: &Request) -> Option<String> {
    request.extensions().get::<String>().cloned()
}