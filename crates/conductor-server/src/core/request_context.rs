use std::future::Future;

use axum::http::Method;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

tokio::task_local! {
    static CURRENT_REQUEST_CONTEXT: RequestContext;
}

/// Correlation data created by the server for one classified API action.
///
/// The context deliberately excludes inbound identifiers, URLs, query strings,
/// headers and bodies. This keeps it safe to attach to authorization decisions
/// and public error responses.
#[derive(Debug, Clone, Serialize)]
pub struct RequestContext {
    pub request_id: Uuid,
    pub route_id: &'static str,
    pub method: &'static str,
    pub occurred_at: DateTime<Utc>,
}

impl RequestContext {
    pub fn new(route_id: &'static str, method: &Method) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            route_id,
            method: normalized_method(method),
            occurred_at: Utc::now(),
        }
    }
}

pub async fn scope<F>(context: RequestContext, future: F) -> F::Output
where
    F: Future,
{
    CURRENT_REQUEST_CONTEXT.scope(context, future).await
}

pub fn current() -> Option<RequestContext> {
    CURRENT_REQUEST_CONTEXT.try_with(Clone::clone).ok()
}

fn normalized_method(method: &Method) -> &'static str {
    match *method {
        Method::GET => "GET",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::PATCH => "PATCH",
        Method::DELETE => "DELETE",
        Method::HEAD => "HEAD",
        Method::OPTIONS => "OPTIONS",
        Method::CONNECT => "CONNECT",
        Method::TRACE => "TRACE",
        _ => "UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn context_is_scoped_and_uses_a_server_generated_id() {
        let context = RequestContext::new("health.read", &Method::GET);
        let expected = context.request_id;

        scope(context, async move {
            let actual = current().expect("request context");
            assert_eq!(actual.request_id, expected);
            assert_eq!(actual.route_id, "health.read");
            assert_eq!(actual.method, "GET");
        })
        .await;

        assert!(current().is_none());
    }
}
