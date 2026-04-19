//! Shared HTTP helpers for API-based collectors.
//!
//! Transient network failures and 429/5xx responses are retried with
//! exponential backoff. Auth/client errors (401, 403, 404, other 4xx) are
//! returned immediately — retrying cannot recover them and a second request
//! only delays the user's error message.

use std::time::Duration;

use reqwest::{RequestBuilder, Response, StatusCode};

/// Max attempts (including the first). Three attempts = up to two retries.
const MAX_ATTEMPTS: u32 = 3;
/// Base backoff; actual delay = BACKOFF_BASE * 2^(attempt-1) → 1s, 2s, 4s.
const BACKOFF_BASE: Duration = Duration::from_secs(1);

fn is_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || matches!(status.as_u16(), 500 | 502 | 503 | 504)
}

/// Clone a request and send it with retry-on-transient-failure semantics.
///
/// The request must be clone-able, which in `reqwest` means the body (if any)
/// must itself be clone-able. Our usage — GET with query params, no body —
/// always is.
pub async fn send_with_retry(request: RequestBuilder) -> reqwest::Result<Response> {
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        // try_clone returns None if the body is a stream; all of our API
        // requests are GETs with no body, so this is safe.
        let this_attempt = match request.try_clone() {
            Some(rb) => rb,
            None => return request.send().await,
        };
        match this_attempt.send().await {
            Ok(resp) => {
                if attempt < MAX_ATTEMPTS && is_retryable_status(resp.status()) {
                    sleep_backoff(attempt).await;
                    continue;
                }
                return Ok(resp);
            }
            Err(err) => {
                if attempt < MAX_ATTEMPTS && is_transient_error(&err) {
                    sleep_backoff(attempt).await;
                    continue;
                }
                return Err(err);
            }
        }
    }
}

fn is_transient_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

async fn sleep_backoff(attempt: u32) {
    let factor = 1u32 << (attempt - 1).min(6);
    tokio::time::sleep(BACKOFF_BASE * factor).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_429_and_5xx_but_not_4xx() {
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_retryable_status(StatusCode::BAD_GATEWAY));
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_retryable_status(StatusCode::GATEWAY_TIMEOUT));

        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED));
        assert!(!is_retryable_status(StatusCode::FORBIDDEN));
        assert!(!is_retryable_status(StatusCode::NOT_FOUND));
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(StatusCode::OK));
    }
}
