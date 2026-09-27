use grw::composite::{Composite, Kinded, TypeSet};
use grw::graph::edge;
use grw::graph::index::{Cardinality, IndexDecl, IndexHit, IndexName, KeyBytes, KeyTag};
use grw::graph::persist;
use grw::graph::{self, MGraph, VGraph};
use grw::layout::Val;
use grw::{Id, id, modify};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Val, serde::Serialize, serde::Deserialize)]
pub enum RelKind {
    Signs,
    Uses,
    Audits,
}

#[derive(Debug, Clone, PartialEq, Eq, Val, serde::Serialize, serde::Deserialize)]
pub struct Rel {
    pub kind: RelKind,
    pub weight: u32,
}

impl Kinded for Rel {
    type Kind = RelKind;

    fn kind(&self) -> RelKind {
        self.kind
    }
}

type Link = TypeSet<Rel>;
type Der = edge::Dir<Link>;

fn rel(kind: RelKind, weight: u32) -> Rel {
    Rel { kind, weight }
}

fn link(parts: &[Rel]) -> Link {
    let (first, rest) = parts.split_first().expect("a link holds at least one part");
    rest.iter().fold(Link::from_part(first.clone()), |held, part| match held.with_part(part.clone()) {
        Ok(grw::composite::Included::Added(next)) => next,
        other => panic!("expected {part:?} to be added, got {other:?}"),
    })
}

const BY_VAL: IndexName = IndexName("by_val");

fn by_val() -> IndexDecl<u32> {
    IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(*v))
}

fn tmp_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("grw_persist_composite_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

const M_0_2_1: &str = "v0_2_1_mgraph_rel.grw";
const V_0_2_1: &str = "v0_2_1_vgraph_rel.grw";

type LinkView = Vec<(Id, Id, <Der as graph::Edge>::Slot, id::E, Vec<Rel>)>;

trait Stored: graph::Graph<u32, Der> {
    fn link_id(&self, a: Id, b: Id, slot: <Der as graph::Edge>::Slot) -> id::E;
}

impl Stored for MGraph<u32, Der> {
    fn link_id(&self, a: Id, b: Id, slot: <Der as graph::Edge>::Slot) -> id::E {
        self.edge_id(<Der as graph::Edge>::edge(slot, (a, b))).expect("every listed link has an id")
    }
}

impl Stored for VGraph<u32, Der> {
    fn link_id(&self, a: Id, b: Id, slot: <Der as graph::Edge>::Slot) -> id::E {
        self.edge_id(<Der as graph::Edge>::edge(slot, (a, b))).expect("every listed link has an id")
    }
}

fn links<G: Stored>(g: &G) -> LinkView {
    let mut v: LinkView = g
        .iter_edges()
        .map(|(a, b, slot, val)| (*a, *b, slot, g.link_id(*a, *b, slot), val.parts().cloned().collect()))
        .collect();
    v.sort_by_key(|(a, b, slot, _, _)| (*a, *b, *slot));
    v
}

type Generations = (Vec<(Id, u64)>, Vec<(id::E, u64)>);

fn generations(g: &VGraph<u32, Der>) -> Generations {
    let nodes: Vec<(Id, u64)> = node_vals(g)
        .into_iter()
        .map(|(n, _)| (n, g.node_gen(id::N(n)).expect("every listed node has a generation")))
        .collect();
    let edges: Vec<(id::E, u64)> = links(g)
        .into_iter()
        .map(|(_, _, _, e, _)| (e, g.edge_gen(e).expect("every listed link has a generation")))
        .collect();
    (nodes, edges)
}

fn one_part_each(view: &LinkView) {
    assert!(view.iter().all(|(_, _, _, _, parts)| parts.len() == 1), "every promoted link holds exactly one part");
}

fn node_vals<E: graph::Edge, G: graph::Graph<u32, E>>(g: &G) -> Vec<(Id, u32)> {
    let mut v: Vec<(Id, u32)> = g.iter_nodes().map(|(n, val, _)| (*n, *val)).collect();
    v.sort_unstable();
    v
}

fn every_value_hits_its_node<E: graph::Edge, G: graph::Graph<u32, E>>(g: &G) {
    let vals = node_vals(g);
    assert!(!vals.is_empty(), "the graph holds nodes to probe");
    for (n, val) in vals {
        match g.index_hit(BY_VAL, &KeyBytes::of(&val), KeyTag::of::<u32>()).unwrap() {
            IndexHit::One(hit) => assert_eq!(hit, id::N(n), "value {val} keys node {n}"),
            _ => panic!("expected one node keyed {val}"),
        }
    }
}

fn refused<T>(result: Result<T, persist::Error>, path: &std::path::Path) -> persist::Reason {
    let Err(err) = result else { panic!("expected {} to be refused", path.display()) };
    assert_eq!(err.path, path, "{err}");
    assert!(err.to_string().starts_with(&path.display().to_string()), "{err}");
    err.reason
}

fn is_edge_layout(reason: persist::Reason) {
    assert!(matches!(reason, persist::Reason::Layout(persist::Layout { side: persist::Side::Edge, .. })), "{reason}");
}

fn is_unrelated(reason: persist::Reason) {
    assert!(matches!(reason, persist::Reason::Promotion(persist::Promotion::Unrelated { .. })), "{reason}");
}

fn is_non_canonical(reason: persist::Reason) {
    let persist::Reason::Value { site: persist::Site::Edge(0), source: bincode::ErrorKind::Custom(msg) } = &reason else {
        panic!("expected the edge value at slot 0 to be refused, got: {reason}")
    };
    assert!(msg.contains("follows part of kind"), "{msg}");
}

fn is_version(reason: persist::Reason, found: u16) {
    assert!(
        matches!(reason, persist::Reason::Format(persist::Format::Version { found: f, expected: 3 }) if f == found),
        "{reason}"
    );
}

fn is_trailer(reason: persist::Reason) {
    assert!(matches!(reason, persist::Reason::Format(persist::Format::TrailerHash)), "{reason}");
}

#[test]
fn fixtures_name_the_part_type() {
    for name in [M_0_2_1, V_0_2_1] {
        let header = persist::read_header(&fixture_path(name)).unwrap();
        assert_eq!(header.version, 3);
        assert_eq!(header.ev_layout_hash, <Rel as Val>::layout_hash());
        assert_ne!(header.ev_layout_hash, <Link as Val>::layout_hash());
    }
}

#[test]
fn type_set_layout_is_its_own() {
    assert_ne!(<Link as Val>::layout_hash(), <Rel as Val>::layout_hash());
    assert_ne!(<Link as Val>::layout_hash(), <smallvec::SmallVec<[Rel; 2]> as Val>::layout_hash());
    assert_ne!(<TypeSet<Rel> as Val>::layout_hash(), <TypeSet<RelOther> as Val>::layout_hash());
    assert_eq!(<Link as Val>::layout_hash(), <Link as Val>::layout_hash());
}

#[test]
fn a_type_set_is_a_set_not_an_array() {
    assert_ne!(<Link as Val>::field_type(), <smallvec::SmallVec<[Rel; 2]> as Val>::field_type());
    assert_eq!(<Link as Val>::field_type(), grw::layout::FieldType::Set(<Rel as Val>::field_type));
}

#[derive(Debug, Clone, PartialEq, Eq, Val, serde::Serialize, serde::Deserialize)]
pub struct RelOther {
    pub kind: RelKind,
    pub weight: u64,
}

impl Kinded for RelOther {
    type Kind = RelKind;

    fn kind(&self) -> RelKind {
        self.kind
    }
}

fn m_0_2_1_as_built_today() -> MGraph<u32, Der> {
    (
        vec![(0, 10u32), (1, 11u32), (2, 12u32), (3, 13u32)],
        vec![
            (edge::dir::E::D(0, 1), link(&[rel(RelKind::Signs, 7)])),
            (edge::dir::E::D(1, 2), link(&[rel(RelKind::Uses, 8)])),
            (edge::dir::E::D(2, 3), link(&[rel(RelKind::Audits, 9)])),
            (edge::dir::E::D(3, 0), link(&[rel(RelKind::Signs, 10)])),
        ],
    )
        .try_into()
        .unwrap()
}

fn v_0_2_1_as_built_today() -> VGraph<u32, Der> {
    let g0: VGraph<u32, Der> = VGraph::new();
    let (g1, _) = g0
        .modify(modify![
            N(0).val(10u32) & E().val(link(&[rel(RelKind::Signs, 7)])) >> N(1).val(11u32),
            n(1) & E().val(link(&[rel(RelKind::Uses, 8)])) >> N(2).val(12u32)
        ])
        .unwrap();
    let (g2, _) = g1.modify(modify![X(2) & E().val(link(&[rel(RelKind::Audits, 9)])) >> X(0)]).unwrap();
    g2
}

#[test]
fn mgraph_0_2_1_fixture_loads_promoted() {
    let g: MGraph<u32, Der> = MGraph::load_promoting_with(&fixture_path(M_0_2_1), vec![by_val()]).unwrap();
    assert_eq!(node_vals(&g), vec![(0, 10), (1, 11), (2, 12), (3, 13)]);
    one_part_each(&links(&g));
    assert_eq!(links(&g).len(), 4);
    assert_eq!(links(&g), links(&m_0_2_1_as_built_today()));
    every_value_hits_its_node(&g);
}

#[test]
fn vgraph_0_2_1_fixture_loads_promoted() {
    let g: VGraph<u32, Der> = VGraph::load_promoting_with(&fixture_path(V_0_2_1), vec![by_val()]).unwrap();
    let today = v_0_2_1_as_built_today();
    assert_eq!(g.version(), 2);
    assert_eq!(node_vals(&g), vec![(0, 10), (1, 11), (2, 12)]);
    one_part_each(&links(&g));
    assert_eq!(links(&g).len(), 3);
    assert_eq!(links(&g), links(&today));
    assert_eq!(generations(&g), generations(&today));
    every_value_hits_its_node(&g);
}

#[test]
fn promoted_0_2_1_graph_saves_as_composite() {
    let g: MGraph<u32, Der> = MGraph::load_promoting_with(&fixture_path(M_0_2_1), vec![by_val()]).unwrap();
    let path = tmp_path("promoted_resaved.grw");
    g.save(&path).unwrap();
    assert_eq!(persist::read_header(&path).unwrap().ev_layout_hash, <Link as Val>::layout_hash());
    let back: MGraph<u32, Der> = MGraph::load_with(&path, vec![by_val()]).unwrap();
    assert_eq!(links(&back), links(&g));
}

#[test]
fn strict_load_of_part_file_is_a_layout_mismatch() {
    let (m, v) = (fixture_path(M_0_2_1), fixture_path(V_0_2_1));
    is_edge_layout(refused(MGraph::<u32, Der>::load_with(&m, vec![by_val()]), &m));
    is_edge_layout(refused(VGraph::<u32, Der>::load_with(&v, vec![by_val()]), &v));
}

#[test]
fn promoting_load_refuses_an_unrelated_edge_value() {
    let g: graph::MDir<u32, u32> = (vec![(0, 1u32), (1, 2u32)], vec![(edge::dir::E::D(0, 1), 5u32)]).try_into().unwrap();
    let path = tmp_path("unrelated.grw");
    g.save(&path).unwrap();
    is_unrelated(refused(MGraph::<u32, Der>::load_promoting(&path), &path));
    is_unrelated(refused(VGraph::<u32, Der>::load_promoting(&path), &path));
}

fn two_part_mgraph() -> MGraph<u32, Der> {
    (
        vec![(0, 10u32), (1, 11u32), (2, 12u32)],
        vec![
            (edge::dir::E::D(0, 1), link(&[rel(RelKind::Uses, 2), rel(RelKind::Signs, 1)])),
            (edge::dir::E::D(1, 2), link(&[rel(RelKind::Audits, 3)])),
            (edge::dir::E::D(2, 0), link(&[rel(RelKind::Audits, 6), rel(RelKind::Signs, 4), rel(RelKind::Uses, 5)])),
        ],
    )
        .try_into()
        .unwrap()
}

fn two_part_vgraph() -> VGraph<u32, Der> {
    let g0: VGraph<u32, Der> = VGraph::new();
    let (g1, _) = g0
        .modify(modify![
            N(0).val(10u32) & E().val(link(&[rel(RelKind::Signs, 1), rel(RelKind::Uses, 2)])) >> N(1).val(11u32),
            n(1) & E().val(link(&[rel(RelKind::Audits, 3)])) >> N(2).val(12u32)
        ])
        .unwrap();
    g1
}

#[test]
fn mgraph_two_part_links_round_trip() {
    let g = two_part_mgraph().with_indices(vec![by_val()]).unwrap();
    let path = tmp_path("m_two_part.grw");
    g.save(&path).unwrap();
    let strict: MGraph<u32, Der> = MGraph::load_with(&path, vec![by_val()]).unwrap();
    let promoting: MGraph<u32, Der> = MGraph::load_promoting_with(&path, vec![by_val()]).unwrap();
    assert_eq!(links(&g).iter().map(|(_, _, _, _, parts)| parts.len()).collect::<Vec<_>>(), vec![2, 3, 1]);
    assert_eq!(links(&strict), links(&g));
    assert_eq!(links(&promoting), links(&g));
    assert_eq!(node_vals(&strict), node_vals(&g));
    every_value_hits_its_node(&strict);
    every_value_hits_its_node(&promoting);
}

#[test]
fn vgraph_two_part_links_round_trip() {
    let g = two_part_vgraph().with_indices(vec![by_val()]).unwrap();
    let path = tmp_path("v_two_part.grw");
    g.save(&path).unwrap();
    let strict: VGraph<u32, Der> = VGraph::load_with(&path, vec![by_val()]).unwrap();
    let promoting: VGraph<u32, Der> = VGraph::load_promoting_with(&path, vec![by_val()]).unwrap();
    assert_eq!(links(&g).iter().map(|(_, _, _, _, parts)| parts.len()).collect::<Vec<_>>(), vec![2, 1]);
    assert_eq!(links(&strict), links(&g));
    assert_eq!(links(&promoting), links(&g));
    assert_eq!(generations(&strict), generations(&g));
    assert_eq!(generations(&promoting), generations(&g));
    assert_eq!(strict.version(), g.version());
    every_value_hits_its_node(&strict);
    every_value_hits_its_node(&promoting);
}

fn resealed(mut bytes: Vec<u8>) -> Vec<u8> {
    let body = bytes.len() - 32;
    let digest = Sha256::digest(&bytes[..body]);
    bytes[body..].copy_from_slice(&digest);
    bytes
}

fn part_bytes(part: &Rel) -> Vec<u8> {
    bincode::serialize(part).unwrap()
}

fn one_non_canonical_link() -> MGraph<u32, Der> {
    (vec![(0, 10u32), (1, 11u32)], vec![(edge::dir::E::D(0, 1), link(&[rel(RelKind::Signs, 1), rel(RelKind::Uses, 2)]))])
        .try_into()
        .unwrap()
}

fn swap_the_parts(path: &std::path::Path) {
    let signs = part_bytes(&rel(RelKind::Signs, 1));
    let uses = part_bytes(&rel(RelKind::Uses, 2));
    let canonical: Vec<u8> = [signs.clone(), uses.clone()].concat();
    let swapped: Vec<u8> = [uses, signs].concat();
    let mut bytes = std::fs::read(path).unwrap();
    let at = bytes.windows(canonical.len()).position(|w| w == canonical.as_slice()).expect("canonical parts in the values section");
    assert_eq!(bytes.windows(canonical.len()).filter(|w| *w == canonical.as_slice()).count(), 1);
    bytes[at..at + swapped.len()].copy_from_slice(&swapped);
    std::fs::write(path, resealed(bytes)).unwrap();
}

#[test]
fn non_canonical_link_is_refused() {
    let path = tmp_path("non_canonical.grw");
    one_non_canonical_link().save(&path).unwrap();
    swap_the_parts(&path);
    is_non_canonical(refused(MGraph::<u32, Der>::load(&path), &path));
    is_non_canonical(refused(MGraph::<u32, Der>::load_promoting(&path), &path));
    is_non_canonical(refused(VGraph::<u32, Der>::load_promoting(&path), &path));
}

#[test]
fn non_canonical_link_is_refused_by_strict_vgraph_load() {
    let path = tmp_path("non_canonical_v.grw");
    VGraph::from_mgraph(&one_non_canonical_link()).save(&path).unwrap();
    swap_the_parts(&path);
    is_non_canonical(refused(VGraph::<u32, Der>::load(&path), &path));
    is_non_canonical(refused(VGraph::<u32, Der>::load_promoting(&path), &path));
}

#[test]
fn unknown_version_is_refused() {
    let mut bytes = std::fs::read(fixture_path(M_0_2_1)).unwrap();
    bytes[4..6].copy_from_slice(&4u16.to_le_bytes());
    let path = tmp_path("unknown_version.grw");
    std::fs::write(&path, resealed(bytes)).unwrap();
    is_version(refused(MGraph::<u32, Der>::load_promoting(&path), &path), 4);
    is_version(refused(VGraph::<u32, Der>::load_promoting(&path), &path), 4);
}

#[test]
fn corrupted_fixture_is_refused() {
    let mut bytes = std::fs::read(fixture_path(V_0_2_1)).unwrap();
    let values = persist::read_header(&fixture_path(V_0_2_1))
        .unwrap()
        .sections
        .into_iter()
        .find(|s| s.tag == persist::SectionTag::Values)
        .expect("values section");
    bytes[values.offset as usize] ^= 0xFF;
    let path = tmp_path("corrupted.grw");
    std::fs::write(&path, bytes).unwrap();
    is_trailer(refused(MGraph::<u32, Der>::load_promoting(&path), &path));
    is_trailer(refused(VGraph::<u32, Der>::load_promoting(&path), &path));
}

#[test]
fn vgraph_0_2_1_fixture_loads_by_its_catalogue_alone() {
    let g: VGraph<u32, Der> = VGraph::load_promoting_catalogued(&fixture_path(V_0_2_1), &[by_val().catalogued()]).unwrap();
    assert_eq!(node_vals(&g), vec![(0, 10), (1, 11), (2, 12)]);
    assert_eq!(links(&g), links(&v_0_2_1_as_built_today()));
    assert_eq!(g.catalogue().iter().count(), 0);
}

#[test]
fn a_catalogue_the_file_does_not_hold_refuses_the_catalogued_load() {
    let other = IndexDecl::new(IndexName("by_other"), Cardinality::Unique, |v: &u32| Some(*v)).catalogued();
    assert!(VGraph::<u32, Der>::load_promoting_catalogued(&fixture_path(V_0_2_1), &[other]).is_err());
    let retagged = IndexDecl::new(BY_VAL, Cardinality::Unique, |v: &u32| Some(u64::from(*v))).catalogued();
    assert!(VGraph::<u32, Der>::load_promoting_catalogued(&fixture_path(V_0_2_1), &[retagged]).is_err());
}
