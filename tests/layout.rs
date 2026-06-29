//! Serpentine-expansion tests: known `(n → depth, face)`, anchor, and `island_len`.

use comiket_tsp::layout::{Block, Dir, Layout, Space};
use comiket_tsp::space::Building;

/// A canonical test island: anchor at the origin, depth running +x, faces split +y.
fn island(n_max: u16, face0_len: Option<u16>) -> Block {
    Block {
        id: "E4-ア".into(),
        block: "ア".into(),
        building: Building::East,
        hall: 4,
        anchor: (0.0, 0.0),
        along: Dir::E,
        cross: Dir::N,
        n_max,
        face0_len,
        pitch: 1.0,
        island_width: 1.5,
    }
}

fn space<'a>(spaces: &'a [Space], id: &str) -> &'a Space {
    spaces
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("missing space {id}"))
}

#[test]
fn even_nmax_serpentine_and_anchor() {
    let block = island(48, None);
    assert_eq!(block.face0_len(), 24);
    assert_eq!(block.island_len(), 23.0);

    let spaces = block.expand();
    assert_eq!(spaces.len(), 96); // 48 numbers × 2 sides

    // n = 1, side a lands exactly on the block anchor.
    let n1a = space(&spaces, "E4-ア-1a");
    assert_eq!(n1a.face, 0);
    assert_eq!(n1a.along, 0.0);
    assert_eq!(n1a.cross, 0.0);
    assert_eq!((n1a.x, n1a.y), (0.0, 0.0));
    assert_eq!(n1a.island_len, 23.0);

    // n = 24 is the last desk on face 0 (far end).
    let n24a = space(&spaces, "E4-ア-24a");
    assert_eq!(n24a.face, 0);
    assert_eq!(n24a.along, 23.0);

    // n = 25 is the first desk on face 1, also at the far end.
    let n25a = space(&spaces, "E4-ア-25a");
    assert_eq!(n25a.face, 1);
    assert_eq!(n25a.along, 23.0);
    assert_eq!(n25a.cross, 1.5);

    // n = 48 is the last desk on face 1, back at the near end.
    let n48a = space(&spaces, "E4-ア-48a");
    assert_eq!(n48a.face, 1);
    assert_eq!(n48a.along, 0.0);
    assert_eq!(n48a.cross, 1.5);

    // The b half sits SIDE_B_OFFSET across from the a half.
    let n1b = space(&spaces, "E4-ア-1b");
    assert_eq!(n1b.cross, 0.45);
}

#[test]
fn odd_nmax_unequal_faces() {
    let block = island(49, None);
    assert_eq!(block.face0_len(), 24); // 49 / 2 == 24
    assert_eq!(block.island_len(), 24.0); // longer face has 25 desks

    let spaces = block.expand();
    assert_eq!(spaces.len(), 98);

    let n25a = space(&spaces, "E4-ア-25a"); // face 1, far end
    assert_eq!(n25a.face, 1);
    assert_eq!(n25a.along, 24.0);

    let n49a = space(&spaces, "E4-ア-49a"); // face 1, near end
    assert_eq!(n49a.face, 1);
    assert_eq!(n49a.along, 0.0);
}

#[test]
fn face0_len_override_splits_faces() {
    let block = island(40, Some(30));
    assert_eq!(block.face0_len(), 30);
    assert_eq!(block.island_len(), 29.0); // longer face is face 0 (30 desks)

    let spaces = block.expand();
    let n30a = space(&spaces, "E4-ア-30a"); // last on face 0 (far end)
    assert_eq!(n30a.face, 0);
    assert_eq!(n30a.along, 29.0);

    let n31a = space(&spaces, "E4-ア-31a"); // first on face 1
    assert_eq!(n31a.face, 1);
    assert_eq!(n31a.along, 9.0); // depth = 40 - 31 + 1 = 10
}

#[test]
fn layout_json_roundtrips_schema() {
    let layout = Layout {
        event: Some("C107".into()),
        spaces: island(48, None).expand(),
    };
    let json = serde_json::to_string(&layout).expect("serialize");
    let back: Layout = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(back.event.as_deref(), Some("C107"));
    assert_eq!(back.spaces.len(), layout.spaces.len());
    let first = &back.spaces[0];
    assert_eq!(first.id, layout.spaces[0].id);
    assert_eq!(first.face, layout.spaces[0].face);
    assert_eq!(first.island_len, layout.spaces[0].island_len);
}
