use std::error::Error;
use std::fmt;
use std::net::{IpAddr, SocketAddr};

use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::NextTimeout;

const LOCALHOST: &str = "localhost";

/// Resolves only loopback destinations, without any name lookup: IP literals and `localhost`.
#[derive(Debug)]
pub(crate) struct LoopbackResolver;

#[derive(Debug)]
pub(crate) struct NotLoopback {
    host: String,
}

impl fmt::Display for NotLoopback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is not a loopback address and this client only talks to the local machine",
            self.host
        )
    }
}

impl Error for NotLoopback {}

impl Resolver for LoopbackResolver {
    fn resolve(
        &self,
        uri: &Uri,
        _config: &Config,
        _timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let host = uri.host().unwrap_or_default();
        let port = uri
            .port_u16()
            .or_else(|| default_port(uri.scheme_str()))
            .ok_or(ureq::Error::HostNotFound)?;
        let ip = loopback_ip(host).ok_or_else(|| {
            ureq::Error::Other(Box::new(NotLoopback {
                host: host.to_owned(),
            }))
        })?;
        let mut addresses = self.empty();
        addresses.push(SocketAddr::new(ip, port));
        Ok(addresses)
    }
}

fn default_port(scheme: Option<&str>) -> Option<u16> {
    match scheme {
        Some("http") => Some(80),
        Some("https") => Some(443),
        _ => None,
    }
}

fn loopback_ip(host: &str) -> Option<IpAddr> {
    if host.eq_ignore_ascii_case(LOCALHOST) {
        return Some(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    }
    let literal = host.trim_start_matches('[').trim_end_matches(']');
    literal.parse::<IpAddr>().ok().filter(IpAddr::is_loopback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_literals_and_localhost_pass() {
        assert!(loopback_ip("127.0.0.1").is_some());
        assert!(loopback_ip("[::1]").is_some());
        assert!(loopback_ip("LocalHost").is_some());
        assert!(loopback_ip("192.0.2.1").is_none());
        assert!(loopback_ip("assets.xboxunity.net").is_none());
    }
}
