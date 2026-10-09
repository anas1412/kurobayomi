//! `UncaughtExceptionInterceptor` (T-12a-005). It is the first application interceptor. A panic anywhere below it
//! becomes `NetError::Other` and the process keeps running. `Canceled` and `Timeout` pass through unchanged.

use async_trait::async_trait;
use futures::FutureExt;
use std::panic::AssertUnwindSafe;

use super::{Chain, Interceptor};
use crate::error::NetError;
use crate::request::Request;
use crate::response::Response;

pub struct UncaughtExceptionInterceptor;

#[async_trait]
impl Interceptor for UncaughtExceptionInterceptor {
    async fn intercept(&self, chain: Chain<'_>, req: Request) -> Result<Response, NetError> {
        match AssertUnwindSafe(chain.proceed(req)).catch_unwind().await {
            Ok(result) => result,
            Err(panic) => Err(NetError::Other(panic_text(panic.as_ref()))),
        }
    }
}

fn panic_text(panic: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = panic.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = panic.downcast_ref::<String>() {
        text.clone()
    } else {
        "interceptor panicked".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::client::NetworkClient;
    use crate::request;

    struct Boom;

    #[async_trait]
    impl Interceptor for Boom {
        async fn intercept(&self, _chain: Chain<'_>, _req: Request) -> Result<Response, NetError> {
            panic!("boom");
        }
    }

    struct Canceled;

    #[async_trait]
    impl Interceptor for Canceled {
        async fn intercept(&self, _chain: Chain<'_>, _req: Request) -> Result<Response, NetError> {
            Err(NetError::Canceled)
        }
    }

    #[tokio::test]
    async fn a_panic_in_a_later_interceptor_becomes_an_other_error() {
        let client = NetworkClient::for_tests(vec![
            Arc::new(UncaughtExceptionInterceptor),
            Arc::new(Boom),
        ]);
        let err = client.execute(request::get("https://x.test/").unwrap()).await.err().unwrap();
        assert!(matches!(err, NetError::Other(ref m) if m == "boom"), "{err:?}");
    }

    #[tokio::test]
    async fn a_panic_leaves_the_process_running_for_the_next_call() {
        let client = NetworkClient::for_tests(vec![
            Arc::new(UncaughtExceptionInterceptor),
            Arc::new(Boom),
        ]);
        for _ in 0..2 {
            assert!(client.execute(request::get("https://x.test/").unwrap()).await.is_err());
        }
    }

    #[tokio::test]
    async fn cancel_and_timeout_pass_through_unchanged() {
        let client = NetworkClient::for_tests(vec![
            Arc::new(UncaughtExceptionInterceptor),
            Arc::new(Canceled),
        ]);
        let err = client.execute(request::get("https://x.test/").unwrap()).await.err().unwrap();
        assert!(matches!(err, NetError::Canceled));
    }
}
