//! Recovery is confined to transport establishment, before login or user input.
use hbb_common::{ResultType, tokio};
use std::{fmt, future::Future};

#[derive(Debug)]
pub struct RelayHandshakeTimeout;
impl fmt::Display for RelayHandshakeTimeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Peer handshake receive timed out: deadline has elapsed")
    }
}
impl std::error::Error for RelayHandshakeTimeout {}

pub async fn retry_before_login<F, Fut, T>(mut attempt: F) -> ResultType<T>
where F: FnMut() -> Fut, Fut: Future<Output = ResultType<T>> {
    for number in 1..=3 {
        match attempt().await {
            Err(error) if number < 3 && error.downcast_ref::<RelayHandshakeTimeout>().is_some() => {
                hbb_common::log::warn!("Relay pre-login handshake timed out; retrying attempt {}/3", number + 1);
                tokio::time::sleep(std::time::Duration::from_millis(number * 200)).await;
            }
            result => return result,
        }
    }
    unreachable!()
}

pub async fn unless_cancelled<F, C>(connection: F, cancelled: C) -> Option<F::Output>
where F: Future, C: Future<Output = ()> {
    tokio::select! {
        biased;
        _ = cancelled => None,
        result = connection => Some(result),
    }
}
