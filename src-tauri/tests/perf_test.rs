//! Pure unit tests for the small helpers introduced by the Rust performance
//! work: tail-window math, bounded top-k selection, reusable blob decoding and
//! exponential polling backoff. No filesystem, DB or network access needed.

use app_lib::commands::diagnostics::{tail_window_start, utf8_leading_continuations};
use app_lib::services::inference::sidecar::next_backoff;
use app_lib::services::library::db::decode_embedding_into;
use app_lib::services::library::search::TopK;
use std::time::Duration;

#[test]
fn tail_window_start_is_bounded_and_saturating() {
    assert_eq!(tail_window_start(1_000, 256), 744);
    assert_eq!(tail_window_start(256, 256), 0);
    assert_eq!(tail_window_start(255, 256), 0);
    assert_eq!(tail_window_start(257, 256), 1);
    assert_eq!(tail_window_start(0, 256), 0);
    assert_eq!(tail_window_start(u64::MAX, 256), u64::MAX - 256);
}

#[test]
fn utf8_leading_continuations_reach_the_next_char_boundary() {
    // "é" is C3 A9. Starting a window on its A9 byte leaves one continuation
    // byte before the ASCII "A" that follows.
    assert_eq!(utf8_leading_continuations(&[0xA9, b'A', b'B']), 1);
    // A 4-byte emoji (F0 9F 98 80): three continuation bytes after the lead.
    assert_eq!(utf8_leading_continuations(&[0x9F, 0x98, 0x80, b'x']), 3);
    // A clean boundary, a leading byte (not a continuation), and empty input.
    assert_eq!(utf8_leading_continuations(&[b'A', b'B']), 0);
    assert_eq!(utf8_leading_continuations(&[0xC3, 0xA9, b'x']), 0);
    assert_eq!(utf8_leading_continuations(&[]), 0);
}

#[test]
fn backoff_doubles_until_it_hits_the_cap() {
    let cap = Duration::from_millis(500);
    let mut delay = Duration::from_millis(50);
    for expected_ms in [100, 200, 400, 500, 500] {
        delay = next_backoff(delay, cap);
        assert_eq!(delay, Duration::from_millis(expected_ms));
    }
}

#[test]
fn top_k_keeps_the_best_scores_in_rank_order() {
    let mut best = TopK::new(3);
    for (score, id) in [
        (0.2, "a"),
        (0.9, "b"),
        (f32::NAN, "not a number"),
        (f32::INFINITY, "infinite"),
        (0.5, "c"),
        (0.1, "d"),
        (0.7, "e"),
    ] {
        best.offer(score, id);
    }
    let sorted = best.into_sorted();
    let ids: Vec<&str> = sorted.iter().map(|(_, id)| id.as_str()).collect();
    assert_eq!(ids, ["b", "e", "c"]);
}

#[test]
fn top_k_breaks_ties_by_arrival_and_evicts_the_worst() {
    // Exact ties keep the first arrivals, so later equal scores are dropped.
    let mut ties = TopK::new(2);
    ties.offer(0.5, "first");
    ties.offer(0.5, "second");
    ties.offer(0.5, "third");
    let kept: Vec<String> = ties.into_sorted().into_iter().map(|(_, id)| id).collect();
    assert_eq!(kept, ["first", "second"]);

    // A better late candidate evicts the worst of a full buffer.
    let mut best = TopK::new(2);
    best.offer(0.1, "low");
    best.offer(0.2, "mid");
    best.offer(0.9, "high");
    let kept: Vec<String> = best.into_sorted().into_iter().map(|(_, id)| id).collect();
    assert_eq!(kept, ["high", "mid"]);

    // A zero-size buffer keeps nothing.
    let mut none = TopK::new(0);
    none.offer(1.0, "x");
    assert!(none.into_sorted().is_empty());
}

#[test]
fn decode_embedding_into_reuses_capacity_and_ignores_remainder() {
    let mut blob = Vec::new();
    for value in [1.0f32, -2.5, 3.0] {
        blob.extend_from_slice(&value.to_le_bytes());
    }
    blob.push(0xAA); // trailing partial f32 must be ignored

    let mut out = Vec::new();
    decode_embedding_into(&blob, &mut out);
    assert_eq!(out, [1.0f32, -2.5, 3.0]);
    let capacity = out.capacity();

    // A second, shorter decode clears the buffer but reuses its allocation.
    decode_embedding_into(&7.0f32.to_le_bytes(), &mut out);
    assert_eq!(out, [7.0f32]);
    assert!(out.capacity() >= capacity);
}
