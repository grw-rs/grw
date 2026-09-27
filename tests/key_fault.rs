use grw::Graph as _;
use grw::graph::edge::Dir;
use grw::graph::error::Index as IndexError;
use grw::graph::index::{Cardinality, IndexDecl, IndexName, TryKeyOf};
use grw::graph::{self};
use grw::modify::error::{Apply, Modify, apply};
use grw::modify::{N, X};

type ER = Dir<()>;
type VG = graph::VDir<u32, ()>;
type MG = graph::MDir<u32, ()>;

const EVEN_ONLY: IndexName = IndexName("even_only");

#[derive(Debug, PartialEq, Eq)]
struct Odd(u32);

impl std::fmt::Display for Odd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is odd", self.0)
    }
}

impl std::error::Error for Odd {}

struct EvenOnly;

impl TryKeyOf<u32> for EvenOnly {
    type Key = u32;
    type Fault = Odd;

    fn key(&self, v: &u32) -> Result<Option<u32>, Odd> {
        match v % 2 {
            0 => Ok(Some(*v)),
            _ => Err(Odd(*v)),
        }
    }
}

fn even_only() -> IndexDecl<u32> {
    IndexDecl::fallible(EVEN_ONLY, Cardinality::Unique, EvenOnly)
}

fn faulted_on(result: Result<(), Modify>, odd: u32) {
    match result {
        Err(Modify::Apply(Apply::Index(apply::Index::KeyFault { index, fault }))) => {
            assert_eq!(index, EVEN_ONLY);
            assert_eq!(fault.fault().downcast_ref::<Odd>(), Some(&Odd(odd)));
            assert_eq!(std::error::Error::source(&fault).and_then(|s| s.downcast_ref::<Odd>()), Some(&Odd(odd)));
        }
        other => panic!("a value the index cannot key refuses the modification, got {other:?}"),
    }
}

fn vals(g: &VG) -> Vec<u32> {
    let mut v: Vec<u32> = g.iter_node_ids().map(|n| *g.node_val(n).unwrap()).collect();
    v.sort_unstable();
    v
}

#[test]
fn a_new_value_the_index_cannot_key_refuses_a_vgraph_modification() {
    let g = VG::new().with_indices(vec![even_only()]).unwrap();
    let (g1, _) = g.modify(vec![N::<u32, ER>(0).val(2u32).into()]).unwrap();
    faulted_on(g1.modify(vec![N::<u32, ER>(0).val(3u32).into()]).map(|_| ()), 3);
    assert_eq!(vals(&g1), vec![2]);
}

#[test]
fn a_swapped_in_value_the_index_cannot_key_refuses_a_vgraph_modification() {
    let g = VG::new().with_indices(vec![even_only()]).unwrap();
    let (g1, m) = g.modify(vec![N::<u32, ER>(0).val(2u32).into()]).unwrap();
    let id = m.new_node_ids.values().copied().next().unwrap();
    faulted_on(g1.modify(vec![X::<u32, ER>(id).val(5u32).into()]).map(|_| ()), 5);
}

#[test]
fn a_new_value_the_index_cannot_key_refuses_an_mgraph_modification_and_leaves_it_whole() {
    let g: MG = grw::mgraph![<u32, ER>; N(0).val(4u32)].unwrap();
    let mut g = g.with_indices(vec![even_only()]).unwrap();
    faulted_on(g.modify(vec![N::<u32, ER>(0).val(7u32).into()]).map(|_| ()), 7);
    assert_eq!(g.iter_node_ids().count(), 1);
}

#[test]
fn declaring_an_index_over_a_value_it_cannot_key_is_refused() {
    let (g, _) = VG::new().modify(vec![N::<u32, ER>(0).val(9u32).into()]).unwrap();
    match g.clone().with_indices(vec![even_only()]) {
        Err(IndexError::KeyFault { index, .. }) => assert_eq!(index, EVEN_ONLY),
        other => panic!("with_indices over an unkeyable value is refused, got {:?}", other.map(|_| ())),
    }
    let mut added = g.clone();
    match added.add_index(even_only()) {
        Err(IndexError::KeyFault { index, .. }) => assert_eq!(index, EVEN_ONLY),
        other => panic!("add_index over an unkeyable value is refused, got {other:?}"),
    }
}
