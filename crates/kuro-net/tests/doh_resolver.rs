//! The DoH resolver against a fake provider (T-12a-025): concurrent A and AAAA queries in GET form, the size limit,
//! the failure text and the reqwest hook.

use std::str::FromStr;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hickory_resolver::proto::op::{Message, MessageType};
use hickory_resolver::proto::rr::rdata::{A, AAAA};
use hickory_resolver::proto::rr::{RData, Record, RecordType};
use kuro_net::doh::DohResolver;
use reqwest::dns::{Name, Resolve};
use url::Url;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

/// Answers each query with one address of its own type, read from the `dns` parameter.
fn answer(req: &Request) -> ResponseTemplate {
    let dns = req
        .url
        .query_pairs()
        .find(|(key, _)| key == "dns")
        .map(|(_, value)| value.into_owned())
        .expect("GET form carries the dns parameter");
    let query = Message::from_vec(&URL_SAFE_NO_PAD.decode(&dns).unwrap()).unwrap();
    let question = &query.queries[0];
    let data = match question.query_type() {
        RecordType::A => RData::A(A("192.0.2.7".parse().unwrap())),
        _ => RData::AAAA(AAAA("2001:db8::7".parse().unwrap())),
    };
    let mut reply = Message::query();
    reply.metadata.id = 0;
    reply.metadata.message_type = MessageType::Response;
    reply.add_answer(Record::from_rdata(question.name().clone(), 60, data));
    ResponseTemplate::new(200).set_body_raw(reply.to_vec().unwrap(), "application/dns-message")
}

fn resolver_for(server: &MockServer, include_ipv6: bool) -> DohResolver {
    let endpoint = Url::parse(&format!("{}/dns-query", server.uri())).unwrap();
    DohResolver::from_parts(endpoint, reqwest::Client::new(), include_ipv6)
}

#[tokio::test]
async fn a_and_aaaa_answers_are_merged_and_the_query_is_get_with_the_dns_media_type() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/dns-query"))
        .and(header("accept", "application/dns-message"))
        .respond_with(answer)
        .expect(2)
        .mount(&server)
        .await;
    let found = resolver_for(&server, true)
        .lookup("example.com")
        .await
        .unwrap();
    assert_eq!(found.len(), 2);
    assert!(found.iter().any(|ip| ip.is_ipv4()) && found.iter().any(|ip| ip.is_ipv6()));
}

#[tokio::test]
async fn with_ipv6_off_only_the_a_query_is_sent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/dns-query"))
        .respond_with(answer)
        .expect(1)
        .mount(&server)
        .await;
    let found = resolver_for(&server, false)
        .lookup("example.com")
        .await
        .unwrap();
    assert_eq!(
        found,
        vec!["192.0.2.7".parse::<std::net::IpAddr>().unwrap()]
    );
}

#[tokio::test]
async fn a_failed_status_is_reported_with_its_code() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/dns-query"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let err = resolver_for(&server, false)
        .lookup("example.com")
        .await
        .unwrap_err();
    assert!(err.to_string().starts_with("response: 500"), "{err}");
}

#[tokio::test]
async fn a_body_over_the_limit_fails_with_its_size() {
    let server = MockServer::start().await;
    let body = vec![0u8; 65_537];
    Mock::given(method("GET"))
        .and(path("/dns-query"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(&server)
        .await;
    let err = resolver_for(&server, false)
        .lookup("example.com")
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "response size exceeds limit (65536 bytes): 65537 bytes"
    );
}

#[tokio::test]
async fn the_reqwest_hook_returns_the_addresses_on_port_zero() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/dns-query"))
        .respond_with(answer)
        .mount(&server)
        .await;
    let resolver = resolver_for(&server, true);
    let addrs: Vec<std::net::SocketAddr> = resolver
        .resolve(Name::from_str("example.com").unwrap())
        .await
        .unwrap()
        .collect();
    assert_eq!(addrs.len(), 2);
    assert!(addrs.iter().all(|addr| addr.port() == 0));
}
