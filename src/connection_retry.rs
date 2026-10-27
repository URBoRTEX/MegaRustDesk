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
#[derive(Debug)]
pub struct RelayHandshakeClosed;
impl fmt::Display for RelayHandshakeClosed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("Peer handshake connection closed before login") }
}
impl std::error::Error for RelayHandshakeClosed {}
pub fn closed_before_login() -> hbb_common::anyhow::Error { RelayHandshakeClosed.into() }
pub fn receive_error(error: std::io::Error, websocket: bool) -> hbb_common::anyhow::Error {
    use std::io::ErrorKind::*;
    let disconnected = matches!(error.kind(), ConnectionReset | ConnectionAborted | UnexpectedEof | BrokenPipe | NotConnected)
        || (websocket && error.kind() == Other && error.to_string().ends_with("Connection reset without closing handshake"));
    if disconnected { hbb_common::anyhow::Error::from(error).context(RelayHandshakeClosed) }
    else { hbb_common::anyhow::Error::from(error).context("Peer handshake receive failed") }
}

pub async fn retry_before_login<F, Fut, T>(mut attempt: F) -> ResultType<T>
where F: FnMut() -> Fut, Fut: Future<Output = ResultType<T>> {
    for number in 1..=3 {
        match attempt().await {
            Err(error) if number < 3 && (error.downcast_ref::<RelayHandshakeTimeout>().is_some() || error.downcast_ref::<RelayHandshakeClosed>().is_some()) => {
                hbb_common::log::warn!("Relay pre-login transport interrupted; retrying attempt {}/3", number + 1);
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
