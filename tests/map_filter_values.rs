use grw::Graph as _;
use grw::graph::edge::Dir;
use grw::graph::error::{Index as IndexError, Map as MapError};
use grw::graph::index::{Cardinality, IndexDecl, IndexName};
use grw::graph::{self};
use grw::id;
use grw::modify::{E, N, X, n, x};

type ER = Dir<u32>;
type VG = graph::VDir<u32, u32>;
type Mapped = graph::VDir<String, String>;

const BY_TEXT: IndexName = IndexName("by_text");

fn by_text() -> IndexDecl<String> {
    IndexDecl::new(BY_TEXT, Cardinality::Unique, |v: &String| Some(v.clone()))
}

fn chain() -> VG {
    let g0: VG = VG::new();
    let (g1, _) = g0
        .modify(vec![
            (N::<u32, ER>(0).val(10u32) & E().val(100u32) >> N(1).val(11u32)).into(),
            (n::<u32, ER>(1) & E().val(101u32) >> N(2).val(12u32)).into(),
        ])
        .expect("the chain builds");
    let (g2, _) = g1.modify(vec![X::<u32, ER>(2).val(22u32).into()]).expect("node 2 changes");
    g2
}

fn texts(g: &Mapped) -> Vec<(u32, String, Option<u64>)> {
    g.iter_node_ids().map(|n| (*n, g.node_val(n).expect("a listed node holds a value").clone(), g.node_gen(n))).collect()
}

fn links(g: &Mapped) -> Vec<(u32, u32, String)> {
    let mut out: Vec<(u32, u32, String)> = g.iter_edges().map(|(lo, hi, _, v)| (*lo, *hi, v.clone())).collect();
    out.sort();
    out
}

fn keep_all(g: &VG) -> Mapped {
    g.map_filter_values::<String, Dir<String>, String>(vec![by_text()], |_, v| Ok(Some(format!("n{v}"))), |_, v| Ok(Some(format!("e{v}"))))
        .expect("every value maps")
}

#[test]
fn every_value_is_mapped_and_ids_generations_links_and_the_version_are_kept() {
    let g = chain();
    let mapped = keep_all(&g);
    assert_eq!(mapped.version(), g.version());
    let born: Vec<Option<u64>> = g.iter_node_ids().map(|n| g.node_gen(n)).collect();
    assert_eq!(texts(&mapped), vec![(0, "n10".to_string(), born[0]), (1, "n11".to_string(), born[1]), (2, "n22".to_string(), born[2])]);
    assert_eq!(links(&mapped), vec![(0, 1, "e100".to_string()), (1, 2, "e101".to_string())]);
    let around: Vec<u32> = mapped.neighbors(id::N(1)).expect("node 1 is held").map(|(nb, _, _)| *nb).collect();
    assert_eq!(around.len(), 2);
    assert!(around.contains(&0) && around.contains(&2), "{around:?}");
    for (lo, hi, _, _) in g.iter_edges() {
        let before = g.edges_between(lo, hi).count();
        assert_eq!(mapped.edges_between(lo, hi).count(), before);
    }
}

#[test]
fn a_dropped_node_takes_its_links_with_it_frees_its_slot_and_keeps_the_version() {
    let g = chain();
    let mapped = g
        .map_filter_values::<String, Dir<String>, String>(
            vec![by_text()],
            |node, v| Ok((*node != 1).then(|| format!("n{v}"))),
            |_, v| Ok(Some(format!("e{v}"))),
        )
        .expect("the map drops node 1");
    assert_eq!(mapped.version(), g.version());
    assert_eq!(texts(&mapped), vec![(0, "n10".to_string(), g.node_gen(id::N(0))), (2, "n22".to_string(), g.node_gen(id::N(2)))]);
    assert!(links(&mapped).is_empty(), "{:?}", links(&mapped));
    assert_eq!(mapped.neighbors(id::N(0)).expect("node 0 is held").count(), 0);
    assert_eq!(mapped.neighbors(id::N(2)).expect("node 2 is held").count(), 0);
    let (next, m) = mapped
        .modify(vec![N::<String, Dir<String>>(9).val("fresh".to_string()).into()])
        .expect("a new node lands after the map");
    assert_eq!(*m.new_node_ids[&grw::modify::LocalId(9)], 1, "the lowest freed slot is taken first");
    assert_eq!(next.node_count(), 3);
}

#[test]
fn a_dropped_edge_keeps_both_ends_and_the_version() {
    let g = chain();
    let mapped = g
        .map_filter_values::<String, Dir<String>, String>(
            vec![by_text()],
            |_, v| Ok(Some(format!("n{v}"))),
            |_, v| Ok((*v != 100).then(|| format!("e{v}"))),
        )
        .expect("the map drops one edge");
    assert_eq!(mapped.version(), g.version());
    assert_eq!(mapped.node_count(), 3);
    assert_eq!(links(&mapped), vec![(1, 2, "e101".to_string())]);
    assert_eq!(mapped.neighbors(id::N(0)).expect("node 0 is held").count(), 0);
    let (next, _) = mapped
        .modify(vec![(x::<String, Dir<String>>(0) & E().val("again".to_string()) >> x(1)).into()])
        .expect("the freed pair links again");
    assert_eq!(next.edges_between(id::N(0), id::N(1)).count(), 1);
}

#[test]
fn a_refused_value_is_the_callers_own_error() {
    let g = chain();
    let refused = g.map_filter_values::<String, Dir<String>, String>(
        vec![by_text()],
        |node, v| match *node {
            2 => Err(format!("node 2 holds {v}")),
            _ => Ok(Some(format!("n{v}"))),
        },
        |_, v| Ok(Some(format!("e{v}"))),
    );
    match refused {
        Err(MapError::Value(said)) => assert_eq!(said, "node 2 holds 22"),
        Err(other) => panic!("expected the caller's error, got {other:?}"),
        Ok(_) => panic!("expected the caller's error, got a graph"),
    }
}

#[test]
fn the_indices_are_built_from_the_declarations_given() {
    let g = chain();
    let collided = g.map_filter_values::<String, Dir<String>, String>(vec![by_text()], |_, _| Ok(Some("same".to_string())), |_, v| Ok(Some(format!("e{v}"))));
    match collided {
        Err(MapError::Index(IndexError::NotUnique { index, nodes })) => {
            assert_eq!(index, BY_TEXT);
            assert_eq!(nodes, vec![id::N(0), id::N(1), id::N(2)]);
        }
        Err(other) => panic!("expected a uniqueness refusal, got {other:?}"),
        Ok(_) => panic!("expected a uniqueness refusal, got a graph"),
    }
    let mapped = keep_all(&g);
    assert_eq!(mapped.catalogue().len(), 1);
}
