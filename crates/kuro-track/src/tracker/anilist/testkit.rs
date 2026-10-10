//! Test doubles for the AniList tracker: a transport that answers from a queue and records each request.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use bytes::Bytes;
use serde_json::Value;

use super::api::{RawResponse, Transport};
use super::error::AniListError;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A 200 response with `body`.
pub fn ok(body: &str) -> RawResponse {
    status(200, body)
}

/// A response with the given status and body.
pub fn status(code: u16, body: &str) -> RawResponse {
    RawResponse {
        status: code,
        body: Bytes::from(body.to_owned()),
        retry_after: None,
    }
}

/// Answers each POST with the next queued response. When the queue is empty it answers `Transient`.
#[derive(Default)]
pub struct FakeTransport {
    queue: Mutex<VecDeque<RawResponse>>,
    seen: Mutex<Vec<(Value, Option<String>)>>,
}

impl FakeTransport {
    pub fn new(responses: Vec<RawResponse>) -> Arc<Self> {
        Arc::new(Self {
            queue: Mutex::new(responses.into()),
            seen: Mutex::new(Vec::new()),
        })
    }

    /// Number of POSTs that reached the transport.
    pub fn calls(&self) -> usize {
        lock(&self.seen).len()
    }

    /// The JSON bodies sent, in order.
    pub fn bodies(&self) -> Vec<Value> {
        lock(&self.seen).iter().map(|(b, _)| b.clone()).collect()
    }

    /// The bearer tokens sent, in order (`None` for an unauthenticated call).
    pub fn bearers(&self) -> Vec<Option<String>> {
        lock(&self.seen).iter().map(|(_, t)| t.clone()).collect()
    }
}

#[async_trait]
impl Transport for FakeTransport {
    async fn post(&self, body: &Value, bearer: Option<&str>) -> Result<RawResponse, AniListError> {
        lock(&self.seen).push((body.clone(), bearer.map(str::to_owned)));
        lock(&self.queue).pop_front().ok_or(AniListError::Transient)
    }
}
