//! Portable public-server transport selection; no system networking changes.
use hbb_common::{config::Config, socket_client, ResultType, Stream};

pub fn public_endpoint(target: &str, proxy: bool) -> Option<String> {
    if proxy { return None; }
    let (host, port) = socket_client::split_host_port(target)?;
    let host = host.to_ascii_lowercase();
    // Accept DNS names only, never URL userinfo, paths or fragments from a peer.
    if host.len() > 253 || host.split('.').any(|label| label.is_empty() || label.len() > 63 || !label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')) {
        return None;
    }
    let path = if host == "rs-ny.rustdesk.com" && (port == 21115 || port == 21116) {
        "/ws/id"
    } else if host.ends_with(".rustdesk.com") && port == 21117 {
        "/ws/relay"
    } else {
        return None;
    };
    Some(format!("wss://{host}{path}"))
}

pub async fn connect_tcp(target: impl std::fmt::Display, timeout: u64) -> ResultType<Stream> {
    let raw = target.to_string();
    let mapped = public_endpoint(&raw, Config::get_socks().is_some());
    let endpoint = mapped.as_ref().unwrap_or(&raw).clone();
    let stream = socket_client::connect_tcp(endpoint, timeout).await?;
    if let Some(endpoint) = mapped { hbb_common::log::info!("Agent transport WSS connected: {endpoint}"); }
    Ok(stream)
}
