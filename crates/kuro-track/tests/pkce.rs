//! T-02: the MAL PKCE verifier shape (D§5.1). 50 random bytes give 67 characters, see `mal::pkce`.

use std::collections::HashSet;

use kuro_track::mal::pkce::{authorize_url, new_verifier};

#[test]
fn t_02_pkce_verifier_shape() {
    let verifiers: HashSet<String> = (0..1000).map(|_| new_verifier()).collect();
    assert_eq!(verifiers.len(), 1000, "all distinct");
    for v in &verifiers {
        assert_eq!(v.len(), 67);
        assert!(
            v.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
    }
    let v = verifiers.iter().next().unwrap();
    let url = authorize_url("cid", v);
    let challenge = url
        .query_pairs()
        .find(|(k, _)| k == "code_challenge")
        .unwrap()
        .1;
    assert_eq!(challenge, v.as_str(), "plain method: challenge == verifier");
}
