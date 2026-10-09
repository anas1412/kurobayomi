//! Download progress (T-12a-012, `ProgressListener.kt`, `ProgressResponseBody.kt`). A progress call wraps the body
//! stream; each chunk reports the running total, and the end of the stream reports `done = true`.

use std::sync::Arc;

use bytes::Bytes;
use futures::{StreamExt, stream};
use http::header::{CONTENT_LENGTH, HeaderMap};

use crate::error::NetError;
use crate::response::BodyStream;

/// Receives progress. `content_length` is `-1` when the server did not send one.
pub trait ProgressListener: Send + Sync {
    fn update(&self, bytes_read: u64, content_length: i64, done: bool);
}

/// The `Content-Length` header as an `i64`, or `-1` when it is missing or not a number.
pub fn content_length_of(headers: &HeaderMap) -> i64 {
    headers
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(-1)
}

/// Wraps `body` so that `listener` sees every chunk and then one final `done = true` call.
pub fn wrap_body(body: BodyStream, content_length: i64, listener: Arc<dyn ProgressListener>) -> BodyStream {
    let state = (body, 0u64, listener);
    stream::unfold(state, move |(mut body, total, listener)| async move {
        match body.next().await {
            Some(Ok(chunk)) => {
                let total = total + chunk.len() as u64;
                listener.update(total, content_length, false);
                Some((Ok::<Bytes, NetError>(chunk), (body, total, listener)))
            }
            Some(Err(err)) => Some((Err(err), (body, total, listener))),
            None => {
                listener.update(total, content_length, true);
                None
            }
        }
    })
    .boxed()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::response::body_from_chunks;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<(u64, i64, bool)>>);

    impl ProgressListener for Recorder {
        fn update(&self, bytes_read: u64, content_length: i64, done: bool) {
            self.0.lock().unwrap().push((bytes_read, content_length, done));
        }
    }

    #[tokio::test]
    async fn progress_reports_done_on_end_of_stream() {
        let rec = Arc::new(Recorder::default());
        let body = wrap_body(body_from_chunks(&[b"abc", b"de"]), 5, rec.clone());
        let collected: Vec<_> = body.collect().await;
        assert_eq!(collected.len(), 2);
        let calls = rec.0.lock().unwrap().clone();
        assert_eq!(calls, vec![(3, 5, false), (5, 5, false), (5, 5, true)]);
    }

    #[tokio::test]
    async fn unknown_length_reports_minus_one() {
        let rec = Arc::new(Recorder::default());
        let len = content_length_of(&HeaderMap::new());
        let body = wrap_body(body_from_chunks(&[b"xy"]), len, rec.clone());
        let _: Vec<_> = body.collect().await;
        let calls = rec.0.lock().unwrap().clone();
        assert_eq!(calls.last(), Some(&(2, -1, true)));
    }

    #[test]
    fn content_length_is_parsed_or_minus_one() {
        let mut h = HeaderMap::new();
        h.insert(CONTENT_LENGTH, "70000".parse().unwrap());
        assert_eq!(content_length_of(&h), 70000);
        h.insert(CONTENT_LENGTH, "nope".parse().unwrap());
        assert_eq!(content_length_of(&h), -1);
    }
}
