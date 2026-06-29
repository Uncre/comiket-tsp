//! Round-trip and rejection tests for canonical space-id parsing.

use std::str::FromStr;

use comiket_tsp::space::{Building, Side, SpaceId};

/// Parse `s`, assert it re-displays to exactly `s`, and return the id.
fn roundtrip(s: &str) -> SpaceId {
    let id = SpaceId::from_str(s).unwrap_or_else(|e| panic!("{s:?} should parse: {e}"));
    assert_eq!(id.to_string(), s, "canonical round-trip for {s:?}");
    id
}

#[test]
fn parses_katakana_block() {
    let id = roundtrip("E4-ア-31a");
    assert_eq!(id.building, Building::East);
    assert_eq!(id.hall, 4);
    assert_eq!(id.block, "ア");
    assert_eq!(id.number, 31);
    assert_eq!(id.side, Side::A);
}

#[test]
fn parses_hiragana_block_side_b() {
    let id = roundtrip("W1-あ-12b");
    assert_eq!(id.building, Building::West);
    assert_eq!(id.hall, 1);
    assert_eq!(id.block, "あ");
    assert_eq!(id.number, 12);
    assert_eq!(id.side, Side::B);
}

#[test]
fn parses_latin_block_and_south() {
    let id = roundtrip("S2-A-7a");
    assert_eq!(id.building, Building::South);
    assert_eq!(id.hall, 2);
    assert_eq!(id.block, "A");
    assert_eq!(id.number, 7);
    assert_eq!(id.side, Side::A);
}

#[test]
fn roundtrips_lowercase_latin_block() {
    roundtrip("E5-t-100b");
}

#[test]
fn accepts_lenient_case_and_normalizes() {
    // Lowercase building and uppercase side are accepted, then normalized.
    let id = SpaceId::from_str("e4-ア-31A").expect("lenient case should parse");
    assert_eq!(id.to_string(), "E4-ア-31a");
}

#[test]
fn rejects_bad_building() {
    assert!(SpaceId::from_str("X4-ア-31a").is_err());
}

#[test]
fn rejects_missing_parts() {
    assert!(SpaceId::from_str("E4-31a").is_err());
    assert!(SpaceId::from_str("E4-ア").is_err());
}

#[test]
fn rejects_bad_side() {
    assert!(SpaceId::from_str("E4-ア-31c").is_err());
}

#[test]
fn rejects_missing_number() {
    assert!(SpaceId::from_str("E4-ア-a").is_err());
}

#[test]
fn rejects_non_numeric_hall() {
    assert!(SpaceId::from_str("Ex-ア-31a").is_err());
}
