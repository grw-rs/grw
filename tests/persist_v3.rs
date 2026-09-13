//! One test per named invariant of the v3 snapshot format
//! (`graph::persist`), `modify_atomicity`-style.

use grw::graph::edge;
use grw::graph::index::{Cardinality, IndexDecl, IndexHit, IndexName, KeyBytes, KeyTag};
use grw::graph::persist::{self, SectionTag};
use grw::graph::{self, MGraph, VGraph};
use grw::{Graph as _, Id, id, modify};

fn tmp_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("grw_persist_v3_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

const BY_VAL: IndexName = IndexName("by_val");
const BY_PARITY: IndexName = IndexName("by_parity");

fn by_val() -> IndexDecl<u32> {
    IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v))
}

fn by_parity() -> IndexDecl<u32> {
    IndexDecl::new(BY_PARITY, Cardinality::Multi, |v: &u32| Some(*v % 2))
}

fn tag() -> KeyTag {
    KeyTag::of::<u32>()
}

fn m_plain_fixture() -> graph::MUndir<u32, u32> {
    (
        vec![(0, 10u32), (1, 11u32), (2, 12u32)],
        vec![(edge::undir::E::U(0, 1), 100u32), (edge::undir::E::U(1, 2), 200u32)],
    )
        .try_into()
        .unwrap()
}

fn m_indexed_fixture() -> graph::MUndir<u32, u32> {
    m_plain_fixture().with_indices(vec![by_val(), by_parity()]).unwrap()
}

fn v_plain_fixture() -> VGraph<u32, edge::Undir<u32>> {
    let g0: VGraph<u32, edge::Undir<u32>> = VGraph::new();
    let (g1, _) = g0
        .modify(modify![
            N(0).val(10u32) & E().val(100u32) ^ N(1).val(11u32),
            n(1) & E().val(200u32) ^ N(2).val(12u32)
        ])
        .unwrap();
    g1
}

fn v_indexed_fixture() -> VGraph<u32, edge::Undir<u32>> {
    v_plain_fixture().with_indices(vec![by_val(), by_parity()]).unwrap()
}

const V_GEN_GOLDEN: &[u8] = include_bytes!("fixtures/golden_v_gen.grw");

/// Node 1 is removed and re-created past it (via node 9 on a later
/// version), so this fixture carries non-zero node generations and a
/// version above 0 — the two things a v2 dump could never store.
fn v_gen_fixture() -> VGraph<u32, edge::Undir<u32>> {
    let g0: VGraph<u32, edge::Undir<u32>> = VGraph::new();
    let (g1, _) =
        g0.modify(modify![N(0).val(10u32) ^ N(1).val(20u32), n(1) ^ N(2).val(30u32)]).unwrap();
    let (g2, _) = g1.modify_versioned(modify![!X(1)], 5).unwrap();
    let (g3, _) = g2.modify_versioned(modify![N(9).val(90u32) ^ x(0)], 8).unwrap();
    g3
}

/// Regeneration path for the v3-gen golden, kept next to its builder for the
/// same reason `persist_golden.rs::bless_golden_fixtures` keeps its
/// builders alongside it. Run `cargo test --test persist_v3 -- --ignored`
/// only when the on-disk format is deliberately changed.
#[test]
#[ignore]
fn bless_v_gen_golden() {
    std::fs::create_dir_all(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")).unwrap();
    v_gen_fixture().save(&fixture_path("golden_v_gen.grw")).unwrap();
}

fn nodes_view<NV: Clone + PartialEq, E: graph::Edge, G: graph::Graph<NV, E>>(g: &G) -> Vec<(Id, NV, Vec<Id>)> {
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

fn edges_view<NV, E: graph::Edge, G: graph::Graph<NV, E>>(g: &G) -> Vec<(Id, Id, E::Slot, E::Val)>
where
    E::Val: Clone + Ord,
{
    let mut v: Vec<(Id, Id, E::Slot, E::Val)> =
        g.iter_edges().map(|(a, b, slot, val)| (*a, *b, slot, val.clone())).collect();
    v.sort();
    v
}

#[test]
fn golden_bytes_v_gen() {
    let path = tmp_path("v_gen_check.grw");
    v_gen_fixture().save(&path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), V_GEN_GOLDEN, "regenerate with `cargo test --test persist_v3 -- --ignored`");
}

#[test]
fn round_trip_m_with_two_indices() {
    let g = m_indexed_fixture();
    let path = tmp_path("m_indexed.grw");
    g.save(&path).unwrap();
    let g2: graph::MUndir<u32, u32> = MGraph::load_with(&path, vec![by_val(), by_parity()]).unwrap();

    assert_eq!(nodes_view(&g2), nodes_view(&g));
    assert_eq!(edges_view(&g2), edges_view(&g));

    match g2.index_hit(BY_VAL, &KeyBytes::of(&11u32), tag()).unwrap() {
        IndexHit::One(n) => assert_eq!(n, id::N(1)),
        _ => panic!("expected One"),
    }
    match g2.index_hit(BY_PARITY, &KeyBytes::of(&0u32), tag()).unwrap() {
        IndexHit::Many(set) => {
            let mut ids: Vec<Id> = set.iter().map(|n| *n).collect();
            ids.sort_unstable();
            assert_eq!(ids, vec![0, 2]);
        }
        _ => panic!("expected Many"),
    }
}

#[test]
fn round_trip_v_with_two_indices() {
    let v = v_indexed_fixture();
    let path = tmp_path("v_indexed.grw");
    v.save(&path).unwrap();
    let v2: VGraph<u32, edge::Undir<u32>> = VGraph::load_with(&path, vec![by_val(), by_parity()]).unwrap();

    assert_eq!(nodes_view(&v2), nodes_view(&v));
    assert_eq!(edges_view(&v2), edges_view(&v));

    match v2.index_hit(BY_VAL, &KeyBytes::of(&12u32), tag()).unwrap() {
        IndexHit::One(n) => assert_eq!(n, id::N(2)),
        _ => panic!("expected One"),
    }
    match v2.index_hit(BY_PARITY, &KeyBytes::of(&1u32), tag()).unwrap() {
        IndexHit::Many(set) => {
            let mut ids: Vec<Id> = set.iter().map(|n| *n).collect();
            ids.sort_unstable();
            assert_eq!(ids, vec![1]);
        }
        _ => panic!("expected Many"),
    }
}

#[test]
fn load_without_decls_when_catalogue_present_names_indices() {
    let g = m_indexed_fixture();
    let path = tmp_path("m_load_no_decls.grw");
    g.save(&path).unwrap();

    let Err(err) = MGraph::<u32, edge::Undir<u32>>::load(&path) else { panic!("expected error") };
    let msg = err.to_string();
    assert!(msg.contains("by_val"), "expected by_val named: {msg}");
    assert!(msg.contains("by_parity"), "expected by_parity named: {msg}");
}

#[test]
fn load_with_wrong_tag_errors() {
    let g = m_indexed_fixture();
    let path = tmp_path("m_wrong_tag.grw");
    g.save(&path).unwrap();

    let wrong_by_val = IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v as u8));
    let Err(err) = MGraph::<u32, edge::Undir<u32>>::load_with(&path, vec![wrong_by_val, by_parity()]) else { panic!("expected error") };
    let msg = err.to_string();
    assert!(msg.contains("by_val"), "expected by_val tag mismatch named: {msg}");
}

#[test]
fn load_with_missing_decl_errors() {
    let g = m_indexed_fixture();
    let path = tmp_path("m_missing_decl.grw");
    g.save(&path).unwrap();

    let Err(err) = MGraph::<u32, edge::Undir<u32>>::load_with(&path, vec![by_val()]) else { panic!("expected error") };
    let msg = err.to_string();
    assert!(msg.contains("by_parity"), "expected missing by_parity named: {msg}");
}

#[test]
fn load_with_extra_decl_errors() {
    let g = m_plain_fixture();
    let path = tmp_path("m_extra_decl.grw");
    g.save(&path).unwrap();

    let Err(err) = MGraph::<u32, edge::Undir<u32>>::load_with(&path, vec![by_val()]) else { panic!("expected error") };
    let msg = err.to_string();
    assert!(msg.contains("by_val"), "expected extra by_val named: {msg}");
}

#[test]
fn flipped_byte_in_each_section_is_trailer_error() {
    let g = m_indexed_fixture();
    let path = tmp_path("m_flip_base.grw");
    g.save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let header = persist::read_header(&path).unwrap();

    for section in &header.sections {
        if matches!(section.tag, SectionTag::Trailer) || section.len == 0 {
            continue;
        }
        let mut bytes = original.clone();
        let idx = section.offset as usize;
        bytes[idx] ^= 0xFF;
        let flipped_path = tmp_path(&format!("m_flip_{:?}.grw", section.tag));
        std::fs::write(&flipped_path, &bytes).unwrap();

        let Err(err) = MGraph::<u32, edge::Undir<u32>>::load(&flipped_path) else { panic!("expected error") };
        let msg = err.to_string();
        assert!(msg.contains("trailer"), "section {:?}: expected trailer error, got: {msg}", section.tag);
        assert!(
            msg.contains(&flipped_path.display().to_string()),
            "section {:?}: expected the file to be named, got: {msg}",
            section.tag
        );
    }
}

#[test]
fn truncated_file_errors_naming_the_file() {
    let g = m_indexed_fixture();
    let path = tmp_path("m_truncate_base.grw");
    g.save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();

    // Cut the file in half: the section table still claims the full ranges,
    // so `start + len` runs past the mapping for every later section.
    let truncated_path = tmp_path("m_truncated.grw");
    std::fs::write(&truncated_path, &original[..original.len() / 2]).unwrap();

    let Err(err) = MGraph::<u32, edge::Undir<u32>>::load(&truncated_path) else { panic!("expected error") };
    let msg = err.to_string();
    assert!(msg.contains("truncated"), "expected a truncation error, got: {msg}");
    assert!(msg.contains(&truncated_path.display().to_string()), "expected the file to be named, got: {msg}");

    // `read_header` walks the same untrusted offsets and must not panic.
    let Err(err) = persist::read_header(&truncated_path) else { panic!("expected error") };
    assert!(
        err.to_string().contains(&truncated_path.display().to_string()),
        "expected the file to be named, got: {err}"
    );
}

/// A section-table entry whose `offset + len` overflows must surface as a
/// named error from every reader, not as a slice-index panic. `read_header`
/// is the one that reaches `section_slice` first (it does not verify the
/// trailer); `load` reaches the same arithmetic inside `verify_trailer`.
#[test]
fn overflowing_section_range_errors_instead_of_panicking() {
    let g = m_indexed_fixture();
    let path = tmp_path("m_bad_offset_base.grw");
    g.save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let header = persist::read_header(&path).unwrap();

    // Each section-table entry is tag + offset + len; find one by its
    // 16 offset/len bytes and replace the offset with u64::MAX.
    let corrupt = |section: &persist::SectionInfo, name: &str| -> std::path::PathBuf {
        let mut needle = section.offset.to_le_bytes().to_vec();
        needle.extend_from_slice(&section.len.to_le_bytes());
        let at = original.windows(needle.len()).position(|w| w == needle).expect("section table entry present");
        let mut bytes = original.clone();
        bytes[at..at + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        let p = tmp_path(name);
        std::fs::write(&p, &bytes).unwrap();
        p
    };

    let indices = header.sections.iter().find(|s| matches!(s.tag, SectionTag::Indices)).unwrap();
    let bad_indices = corrupt(indices, "m_bad_indices_offset.grw");
    let Err(err) = persist::read_header(&bad_indices) else { panic!("expected error") };
    let msg = err.to_string();
    assert!(msg.contains(&bad_indices.display().to_string()), "expected the file to be named, got: {msg}");

    let trailer = header.sections.iter().find(|s| matches!(s.tag, SectionTag::Trailer)).unwrap();
    let bad_trailer = corrupt(trailer, "m_bad_trailer_offset.grw");
    let Err(err) = MGraph::<u32, edge::Undir<u32>>::load(&bad_trailer) else { panic!("expected error") };
    let msg = err.to_string();
    assert!(msg.contains(&bad_trailer.display().to_string()), "expected the file to be named, got: {msg}");
}

fn dir_fixture_v2() -> graph::MDir0 {
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

#[test]
fn v2_fixture_load_errors_naming_convert() {
    let path = fixture_path("golden_v2_dir.grw");
    let Err(err) = MGraph::<(), edge::Dir<()>>::load(&path) else { panic!("expected error") };
    let msg = err.to_string();
    assert!(msg.contains("convert"), "expected convert hint: {msg}");
}

#[test]
fn convert_then_load_equals_original() {
    let from = fixture_path("golden_v2_dir.grw");
    let to = tmp_path("converted_dir.grw");
    let report = persist::convert::<(), edge::Dir<()>>(&from, &to).unwrap();

    let original = dir_fixture_v2();
    assert_eq!(report.node_count, original.node_count() as u64);
    assert_eq!(report.edge_count, original.edge_count() as u64);

    let g: graph::MDir0 = MGraph::load(&to).unwrap();
    assert_eq!(nodes_view(&g), nodes_view(&original));
    assert_eq!(edges_view(&g), edges_view(&original));
}

#[test]
fn v_save_m_load_drops_gens_then_v_load_resets() {
    let v = v_gen_fixture();
    let path = tmp_path("v_then_m_then_v.grw");
    v.save(&path).unwrap();

    let m: graph::MUndir<u32, u32> = MGraph::load(&path).unwrap();
    assert_eq!(nodes_view(&m), nodes_view(&v));
    assert_eq!(edges_view(&m), edges_view(&v));

    m.save(&path).unwrap();
    let v2: VGraph<u32, edge::Undir<u32>> = VGraph::load(&path).unwrap();
    assert_eq!(v2.version(), 0);
    for n in v2.iter_node_ids() {
        assert_eq!(v2.node_gen(n), Some(0), "node {n:?} must reset to generation 0");
    }
}

#[test]
fn read_header_reports_sections_and_catalogue() {
    let g = m_indexed_fixture();
    let path = tmp_path("m_header.grw");
    g.save(&path).unwrap();

    let header = persist::read_header(&path).unwrap();
    assert_eq!(header.version, 3);
    assert_eq!(header.graph_kind, 0);
    assert_eq!(header.edge_kind, 0);
    assert_eq!(header.sections.len(), 7);

    let mut tags: Vec<SectionTag> = header.sections.iter().map(|s| s.tag).collect();
    tags.sort_by_key(|t| format!("{t:?}"));
    let mut expected = vec![
        SectionTag::Nodes,
        SectionTag::Edges,
        SectionTag::Adjacency,
        SectionTag::Values,
        SectionTag::Free,
        SectionTag::Indices,
        SectionTag::Trailer,
    ];
    expected.sort_by_key(|t| format!("{t:?}"));
    assert_eq!(tags, expected);

    let mut names: Vec<&str> = header.catalogue.iter().map(|c| c.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["by_parity", "by_val"]);
}

#[test]
fn save_is_deterministic() {
    let g = m_indexed_fixture();
    let p1 = tmp_path("det_m_1.grw");
    let p2 = tmp_path("det_m_2.grw");
    g.save(&p1).unwrap();
    g.save(&p2).unwrap();
    assert_eq!(std::fs::read(&p1).unwrap(), std::fs::read(&p2).unwrap());

    let v = v_indexed_fixture();
    let p3 = tmp_path("det_v_1.grw");
    let p4 = tmp_path("det_v_2.grw");
    v.save(&p3).unwrap();
    v.save(&p4).unwrap();
    assert_eq!(std::fs::read(&p3).unwrap(), std::fs::read(&p4).unwrap());
}
