//! Pure tests for the share pairing password: generation, hashing,
//! comparison and the one-time URL builder. No network and no user files.

use app_lib::services::share::pairing::{
    TOKEN_HEX_LEN, generate_token, hash_token, pairing_url, token_from_query, token_matches,
};

#[test]
fn tokens_are_random_hex_of_the_expected_shape() {
    let a = generate_token().unwrap();
    let b = generate_token().unwrap();
    assert_ne!(a, b, "two runs must not make the same token");
    assert_eq!(a.len(), TOKEN_HEX_LEN);
    assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(a, a.to_lowercase(), "hex is lowercase and URL-safe");
}

#[test]
fn only_the_matching_token_passes_its_hash() {
    let token = generate_token().unwrap();
    let hash = hash_token(&token);
    assert!(token_matches(&hash, &token));
    assert!(
        token_matches(&hash, &format!("  {token}  ")),
        "header values are trimmed"
    );
    assert!(!token_matches(&hash, ""));
    assert!(!token_matches(&hash, &token.to_uppercase()));
    let other = generate_token().unwrap();
    assert!(!token_matches(&hash, &other));
    let mut changed = token.clone();
    changed.replace_range(0..1, if token.starts_with('0') { "1" } else { "0" });
    assert!(!token_matches(&hash, &changed));
    assert_ne!(hash_token(&token), hash_token(&other), "hashes differ too");
}

#[test]
fn pairing_url_carries_one_token_query_parameter() {
    let token = generate_token().unwrap();
    let url = pairing_url("http://127.0.0.1:4317/", &token);
    assert_eq!(url, format!("http://127.0.0.1:4317/?token={token}"));
    assert_eq!(token_from_query(&url).as_deref(), Some(token.as_str()));

    let tunnel = pairing_url("https://example.trycloudflare.com", &token);
    assert_eq!(
        tunnel,
        format!("https://example.trycloudflare.com/?token={token}")
    );
    assert_eq!(token_from_query(&tunnel).as_deref(), Some(token.as_str()));

    assert_eq!(token_from_query("http://x/p?t=abc"), None);
    assert_eq!(token_from_query("http://x/p"), None);
    assert_eq!(token_from_query(&format!("http://x/p?token={token}#frag")).as_deref(), Some(token.as_str()));
}
