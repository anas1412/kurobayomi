//! Requests to the internet only: sources are written by anyone, and none may reach this computer or the local network
//! (a router's admin page, a service on 127.0.0.1). The app's own client refuses every address that is not public, after
//! the name is resolved (so a public name that points at 192.168.1.1 is refused too), for an address written in the URL,
//! and at each redirect.

use std::net::{IpAddr, SocketAddr};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::redirect::Policy;
use url::Url;

use crate::doh::DohResolver;

/// The error text of a refused address. Sources see it as their request's error.
pub const REFUSED: &str = "address not allowed: sources may only reach the internet, not this computer or the local network";

/// Whether `ip` is on the public internet: not this computer, a private or link-local network, carrier NAT, multicast,
/// broadcast, documentation or unspecified.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                // 100.64.0.0/10, carrier-grade NAT; 0.0.0.0/8, "this network"
                || (a == 100 && (64..128).contains(&b))
                || a == 0)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // fc00::/7 unique local, fe80::/10 link-local
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80)
        }
    }
}

/// Whether `url` may be asked: an address written in it must be public. A name is checked when it is resolved
/// ([`PublicResolver`]).
pub fn url_allowed(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(ip)) => is_public(IpAddr::V4(ip)),
        Some(url::Host::Ipv6(ip)) => is_public(IpAddr::V6(ip)),
        Some(url::Host::Domain(_)) => true,
        None => false,
    }
}

/// Follows at most `max` redirects, and none to an address that is not public. A name a redirect leads to is checked
/// by the resolver when it is connected to.
pub fn redirect_policy(max: usize) -> Policy {
    Policy::custom(move |attempt| {
        if attempt.previous().len() >= max {
            attempt.error("too many redirects")
        } else if !url_allowed(attempt.url()) {
            attempt.error(REFUSED)
        } else {
            attempt.follow()
        }
    })
}

/// Whether something outside this client (the video player) may be given `url`: an http or https link whose host is
/// on the internet, by its written address or by every address its name resolves to.
pub async fn link_allowed(url: &Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") || !url_allowed(url) {
        return false;
    }
    let Some(url::Host::Domain(name)) = url.host() else { return true };
    let port = url.port_or_known_default().unwrap_or(443);
    match tokio::net::lookup_host((name, port)).await {
        Ok(found) => {
            let found: Vec<SocketAddr> = found.collect();
            !found.is_empty() && found.iter().all(|addr| is_public(addr.ip()))
        }
        // a name that does not resolve cannot reach anything; the player reports it as it would any dead link
        Err(_) => true,
    }
}

/// Resolves names (through DNS over HTTPS when it is on, else the system) and keeps only public addresses. A name with
/// none left is an error, so nothing is sent.
#[derive(Clone)]
pub struct PublicResolver {
    pub doh: Option<DohResolver>,
}

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let doh = self.doh.clone();
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let found: Vec<SocketAddr> = match doh {
                Some(doh) => doh.resolve(name).await?.collect(),
                None => tokio::net::lookup_host((host.as_str(), 0)).await?.collect(),
            };
            let public: Vec<SocketAddr> = found.into_iter().filter(|addr| is_public(addr.ip())).collect();
            if public.is_empty() {
                return Err(format!("{host}: {REFUSED}").into());
            }
            let addrs: Addrs = Box::new(public.into_iter());
            Ok(addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_internet_addresses_are_public() {
        for local in ["127.0.0.1", "10.1.2.3", "172.16.0.1", "192.168.1.1", "169.254.169.254", "100.64.0.1", "0.0.0.0", "::1", "fe80::1", "fd00::1", "::ffff:192.168.1.1", "224.0.0.1"] {
            assert!(!is_public(local.parse().unwrap()), "{local} is not on the internet");
        }
        for public in ["1.1.1.1", "93.184.216.34", "172.32.0.1", "2606:4700:4700::1111"] {
            assert!(is_public(public.parse().unwrap()), "{public} is on the internet");
        }
    }

    #[test]
    fn an_address_written_in_the_url_is_checked_and_a_name_waits_for_its_lookup() {
        let allowed = |url: &str| url_allowed(&Url::parse(url).unwrap());
        assert!(!allowed("http://192.168.1.1/admin"));
        assert!(!allowed("http://[::1]:8080/"));
        assert!(allowed("https://1.1.1.1/"));
        assert!(allowed("https://example.com/"), "a name is checked when it is resolved");
    }

    #[tokio::test]
    async fn a_link_for_the_player_must_be_web_and_on_the_internet() {
        let allowed = |url: &str| {
            let url = Url::parse(url).unwrap();
            async move { link_allowed(&url).await }
        };
        assert!(!allowed("file:///home/me/.ssh/id_ed25519").await);
        assert!(!allowed("av://lavfi/testsrc").await);
        assert!(!allowed("http://192.168.1.1/a.mp4").await);
        assert!(!allowed("http://localhost:8080/a.m3u8").await, "a name that resolves to this computer");
        assert!(allowed("https://1.1.1.1/a.mp4").await);
    }

    #[tokio::test]
    async fn a_name_that_resolves_only_to_this_computer_is_refused() {
        let resolver = PublicResolver { doh: None };
        let refused = resolver.resolve("localhost".parse().unwrap()).await;
        assert!(refused.err().is_some_and(|e| e.to_string().contains("address not allowed")));
    }
}
