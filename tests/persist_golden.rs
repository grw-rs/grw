use grw::graph::{self, Graph as _, MGraph, edge};
use grw::{Id, modify};

fn undir_fixture() -> graph::MUndir<u32, u32> {
    use edge::undir::E::U;
    let mut g: graph::MUndir<u32, u32> = (
        vec![(0, 10u32), (1, 20u32), (2, 30u32), (3, 40u32)],
        vec![(U(0, 1), 100u32), (U(1, 2), 200u32), (U(2, 3), 300u32)],
    )
        .try_into()
        .unwrap();
    g.modify(modify![x(1) & !e() ^ x(2)]).unwrap();
    g.modify(modify![x(0) & E().val(400u32) ^ x(3)]).unwrap();
    g.modify(modify![!X(3)]).unwrap();
    g.modify(modify![N(9).val(90u32) & E().val(500u32) ^ x(0)]).unwrap();
    g.modify(modify![N(11).val(110u32)]).unwrap();
    g.modify(modify![!X(1)]).unwrap();
    g
}

fn dir_fixture() -> graph::MDir0 {
    use edge::dir::E::D;
    let mut g: graph::MDir0 = vec![D(0, 1), D(1, 2), D(2, 3)].try_into().unwrap();
    g.modify(modify![x(1) & !e() >> x(2)]).unwrap();
    g.modify(modify![x(0) >> x(3)]).unwrap();
    g.modify(modify![!X(2)]).unwrap();
    g.modify(modify![N(9) >> x(0)]).unwrap();
    g.modify(modify![N(11)]).unwrap();
    g.modify(modify![!X(1)]).unwrap();
    g
}

fn anydir_fixture() -> graph::MAnydir0 {
    use edge::anydir::E::{D, U};
    let mut g: graph::MAnydir0 = vec![U(0, 1), D(1, 2), U(2, 3)].try_into().unwrap();
    g.modify(modify![x(1) & !e() >> x(2)]).unwrap();
    g.modify(modify![x(0) ^ x(3)]).unwrap();
    g.modify(modify![!X(2)]).unwrap();
    g.modify(modify![N(9) >> x(0)]).unwrap();
    g.modify(modify![N(11)]).unwrap();
    g.modify(modify![!X(1)]).unwrap();
    g
}

const UNDIR_GOLDEN: &[u8] = include_bytes!("fixtures/golden_undir.grw");
const DIR_GOLDEN: &[u8] = include_bytes!("fixtures/golden_dir.grw");
const ANYDIR_GOLDEN: &[u8] = include_bytes!("fixtures/golden_anydir.grw");

fn tmp_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("grw_golden_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn saved_bytes<NV, E: grw::graph::Edge>(g: &MGraph<NV, E>, name: &str) -> Vec<u8>
where
    NV: serde::Serialize + grw::layout::Val,
    E::Slot: serde::Serialize,
    E::Val: serde::Serialize + grw::layout::Val,
{
    let path = tmp_path(name);
    g.save(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    bytes
}

/// Regeneration path for the golden files, kept next to the builders so the
/// pinned bytes and the graphs that produce them cannot drift apart. Excluded
/// from `cargo test`; run `cargo test --test persist_golden -- --ignored` only
/// when the on-disk format is deliberately changed.
#[test]
#[ignore]
fn bless_golden_fixtures() {
    std::fs::create_dir_all(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
    )
    .unwrap();
    undir_fixture().save(&fixture_path("golden_undir.grw")).unwrap();
    dir_fixture().save(&fixture_path("golden_dir.grw")).unwrap();
    anydir_fixture().save(&fixture_path("golden_anydir.grw")).unwrap();
}

/// Edge tombstones are not observable through any public accessor: the edge
/// store width lives behind `Edges`. The compaction delta stands in for it —
/// `from_graph` reproduces the node store verbatim and only drops vacant edge
/// slots, so a strictly shorter generic write means the source held them.
fn compaction_delta<NV, E: grw::graph::Edge>(g: &MGraph<NV, E>, name: &str) -> usize
where
    NV: Clone + serde::Serialize + grw::layout::Val,
    E::Slot: serde::Serialize,
    E::Val: Clone + serde::Serialize + grw::layout::Val,
{
    let direct = tmp_path(&format!("{name}_direct.grw"));
    let generic = tmp_path(&format!("{name}_generic.grw"));
    g.save(&direct).unwrap();
    graph::persist::save_graph(g, &generic).unwrap();
    let d = std::fs::metadata(&direct).unwrap().len() as usize;
    let c = std::fs::metadata(&generic).unwrap().len() as usize;
    std::fs::remove_file(&direct).unwrap();
    std::fs::remove_file(&generic).unwrap();
    assert!(d >= c, "compaction must not grow the file: {d} < {c}");
    d - c
}

#[test]
fn fixtures_carry_tombstones() {
    let u = undir_fixture();
    assert_eq!(u.node_count(), 4);
    assert_eq!(u.id_space_len(), 5);
    assert_eq!(u.edge_count(), 1);
    assert!(compaction_delta(&u, "undir") > 0, "undir fixture lost its edge tombstones");

    let d = dir_fixture();
    assert_eq!(d.node_count(), 4);
    assert_eq!(d.id_space_len(), 5);
    assert_eq!(d.edge_count(), 2);
    assert!(compaction_delta(&d, "dir") > 0, "dir fixture lost its edge tombstones");

    let a = anydir_fixture();
    assert_eq!(a.node_count(), 4);
    assert_eq!(a.id_space_len(), 5);
    assert_eq!(a.edge_count(), 2);
    assert!(compaction_delta(&a, "anydir") > 0, "anydir fixture lost its edge tombstones");
}

fn gap_free_undir() -> graph::MUndir<u32, u32> {
    use edge::undir::E::U;
    (
        vec![(0, 10u32), (1, 20u32), (2, 30u32), (3, 40u32)],
        vec![(U(0, 1), 100u32), (U(1, 2), 200u32), (U(2, 3), 300u32), (U(3, 0), 400u32)],
    )
        .try_into()
        .unwrap()
}

fn assert_generic_write_is_byte_identical<NV, E: grw::graph::Edge>(
    g: &MGraph<NV, E>,
    name: &str,
) where
    NV: Clone + serde::Serialize + grw::layout::Val,
    E::Slot: serde::Serialize,
    E::Val: Clone + serde::Serialize + grw::layout::Val,
{
    let direct = tmp_path(&format!("{name}_gapfree_direct.grw"));
    let generic = tmp_path(&format!("{name}_gapfree_generic.grw"));
    g.save(&direct).unwrap();
    graph::persist::save_graph(g, &generic).unwrap();
    assert_eq!(
        std::fs::read(&direct).unwrap(),
        std::fs::read(&generic).unwrap(),
        "{name}: generic writer must reproduce the direct bytes on a gap-free graph",
    );
    std::fs::remove_file(&direct).unwrap();
    std::fs::remove_file(&generic).unwrap();
}

#[test]
fn generic_write_byte_identical_when_gap_free() {
    use edge::anydir::E::{D as AD, U as AU};
    use edge::dir::E::D;

    let u = gap_free_undir();
    assert_eq!(u.node_count(), 4);
    assert_eq!(u.id_space_len(), 4);
    assert_eq!(compaction_delta(&u, "gapfree_undir"), 0);
    assert_generic_write_is_byte_identical(&u, "undir");

    let d: graph::MDir0 = vec![D(0, 1), D(1, 2), D(2, 3), D(3, 0)].try_into().unwrap();
    assert_eq!(d.id_space_len(), 4);
    assert_generic_write_is_byte_identical(&d, "dir");

    let a: graph::MAnydir0 =
        vec![AU(0, 1), AD(1, 2), AU(2, 3), AD(3, 0)].try_into().unwrap();
    assert_eq!(a.id_space_len(), 4);
    assert_generic_write_is_byte_identical(&a, "anydir");
}

#[test]
fn golden_bytes_undir() {
    assert_eq!(saved_bytes(&undir_fixture(), "undir.grw"), UNDIR_GOLDEN);
}

#[test]
fn golden_bytes_dir() {
    assert_eq!(saved_bytes(&dir_fixture(), "dir.grw"), DIR_GOLDEN);
}

#[test]
fn golden_bytes_anydir() {
    assert_eq!(saved_bytes(&anydir_fixture(), "anydir.grw"), ANYDIR_GOLDEN);
}

fn nodes_view<NV: Clone + PartialEq, E: grw::graph::Edge, G: graph::Graph<NV, E>>(
    g: &G,
) -> Vec<(Id, NV, Vec<Id>)> {
    let mut v: Vec<(Id, NV, Vec<Id>)> = g
        .iter_nodes()
        .map(|(n, val, adj)| {
            let mut nbs: Vec<Id> = adj.map(|nb| *nb).collect();
            nbs.sort_unstable();
            (*n, val.clone(), nbs)
        })
        .collect();
    v.sort_by_key(|(n, _, _)| *n);
    v
}

/// Left in `iter_edges` order rather than sorted: every caller compares two
/// `MGraph`s, whose edge order is the store order the format pins.
fn edge_shape<NV, E: grw::graph::Edge, G: graph::Graph<NV, E>>(
    g: &G,
) -> Vec<(Id, Id, E::Slot, E::Val)>
where
    E::Val: Clone,
{
    g.iter_edges().map(|(a, b, slot, val)| (*a, *b, slot, val.clone())).collect()
}

#[test]
fn generic_writer_compacts_edge_tombstones() {
    let u = undir_fixture();
    let direct = tmp_path("direct.grw");
    let generic = tmp_path("generic.grw");
    u.save(&direct).unwrap();
    graph::persist::save_graph(&u, &generic).unwrap();

    assert_ne!(std::fs::read(&direct).unwrap(), std::fs::read(&generic).unwrap());

    let back: graph::MUndir<u32, u32> = MGraph::load(&generic).unwrap();
    assert_eq!(nodes_view(&back), nodes_view(&u));
    assert_eq!(edge_shape(&back), edge_shape(&u));
    assert_eq!(back.id_space_len(), u.id_space_len());

    std::fs::remove_file(&direct).unwrap();
    std::fs::remove_file(&generic).unwrap();
}

#[test]
fn golden_fixtures_load_back() {
    let u: graph::MUndir<u32, u32> = MGraph::load(&fixture_path("golden_undir.grw")).unwrap();
    assert_eq!(nodes_view(&u), nodes_view(&undir_fixture()));
    assert_eq!(edge_shape(&u), edge_shape(&undir_fixture()));
    assert_eq!(u.id_space_len(), undir_fixture().id_space_len());

    let d: graph::MDir0 = MGraph::load(&fixture_path("golden_dir.grw")).unwrap();
    assert_eq!(nodes_view(&d), nodes_view(&dir_fixture()));
    assert_eq!(edge_shape(&d), edge_shape(&dir_fixture()));
    assert_eq!(d.id_space_len(), dir_fixture().id_space_len());

    let a: graph::MAnydir0 = MGraph::load(&fixture_path("golden_anydir.grw")).unwrap();
    assert_eq!(nodes_view(&a), nodes_view(&anydir_fixture()));
    assert_eq!(edge_shape(&a), edge_shape(&anydir_fixture()));
    assert_eq!(a.id_space_len(), anydir_fixture().id_space_len());
}

#[test]
fn golden_undir_payload_literals() {
    use edge::undir::E::U;
    let u: graph::MUndir<u32, u32> = MGraph::load(&fixture_path("golden_undir.grw")).unwrap();
    assert_eq!(
        nodes_view(&u),
        vec![
            (0, 10u32, vec![3]),
            (2, 30u32, vec![]),
            (3, 90u32, vec![0]),
            (4, 110u32, vec![]),
        ],
    );
    assert_eq!(u.get(U(0, 3)), Some(&500u32));
    assert_eq!(u.edge_iter().map(|(_, v)| *v).collect::<Vec<u32>>(), vec![500u32]);
}
