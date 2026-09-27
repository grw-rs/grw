use composite_downstream::{KeepHollow, NothingToExclude, OtherPartHeld, Rel, RelKind, Signs, Uses};
use grw::composite::{Composite, Included, Removed, Typed};

fn signs(since: u32) -> Signs {
    Signs { since }
}

fn uses(service: &str) -> Uses {
    Uses { service: service.to_string() }
}

fn added<C: Composite>(included: Included<C>) -> C {
    match included {
        Included::Added(c) => c,
        Included::Held => panic!("expected a new part to be added"),
    }
}

fn remains<C: Composite>(removed: Removed<C>) -> C {
    match removed {
        Removed::Remains(c) => c,
        Removed::Vacant => panic!("expected the value to remain"),
    }
}

#[test]
fn last_exclusion_leaves_a_hollow_value() {
    let one = KeepHollow::from_part(Rel::Signs(signs(1)));
    let hollow = remains(one.exclude::<Signs>().expect("held"));
    assert!(hollow.is_hollow());
    assert_eq!(hollow.parts().count(), 0);
    assert_eq!(hollow.get::<Signs>(), None);
    let refilled = added(hollow.include(uses("svc")).expect("new kind"));
    assert_eq!(refilled.get::<Uses>(), Some(&uses("svc")));
}

#[test]
fn the_combinator_names_its_own_refusals() {
    let one = KeepHollow::from_part(Rel::Signs(signs(1)));
    assert_eq!(one.include(signs(5)), Err(OtherPartHeld { kind: RelKind::Signs }));
    assert_eq!(one.exclude::<Uses>(), Err(NothingToExclude { kind: RelKind::Uses }));
}

#[test]
fn typed_projection_works_on_both_combinators() {
    let hollow = added(KeepHollow::from_part(Rel::Uses(uses("svc"))).include(signs(3)).expect("new"));
    let set = added(grw::TypeSet::from_part(Rel::Uses(uses("svc"))).include(signs(3)).expect("new"));
    assert_eq!(hollow.get::<Signs>(), Some(&signs(3)));
    assert_eq!(set.get::<Signs>(), Some(&signs(3)));
    assert_eq!(hollow.get::<Uses>(), Some(&uses("svc")));
    assert_eq!(set.get::<Uses>(), Some(&uses("svc")));
}

#[test]
fn excluding_the_last_part_keeps_a_hollow_link_in_the_graph() {
    use grw::Graph;
    use grw::edge::{Dir, dir};
    use grw::modify::{ExcludePart, Node, edge, node};

    type ER = Dir<KeepHollow<Rel>>;

    let exclude_signs = || -> Vec<Node<(), ER>> {
        vec![Node::Exist(node::Exist::Bind {
            id: grw::id::N(0),
            op: node::Bind::Ref,
            edges: vec![edge::Edge::Part {
                slot: dir::SRC,
                op: ExcludePart::new(RelKind::Signs).into(),
                target: Node::Exist(node::Exist::Bind { id: grw::id::N(1), op: node::Bind::Ref, edges: vec![] }),
            }],
        })]
    };
    let mut g: grw::MGraph<(), ER> =
        (2, vec![(dir::E::D(0, 1), KeepHollow::from_part(Rel::Signs(signs(1))))]).try_into().unwrap();
    let v = grw::VGraph::from_mgraph(&g);

    let m = g.modify(exclude_signs()).expect("the held part is excluded");
    let (v, vm) = v.modify(exclude_signs()).expect("the held part is excluded");

    for (graph_edges, removed, swapped, hollow) in [
        (g.edge_count(), m.removed_edges.len(), m.swapped_edge_vals.len(), g.get_edge_val(dir::E::D(0, 1))),
        (v.edge_count(), vm.removed_edges.len(), vm.swapped_edge_vals.len(), v.get_edge_val(dir::E::D(0, 1))),
    ] {
        assert_eq!((graph_edges, removed, swapped), (1, 0, 1));
        assert!(hollow.expect("the link stays").is_hollow());
    }
}

mod dsl {
    use super::*;
    use grw::Graph;
    use grw::graph::edge::Dir;
    use grw::{mgraph, modify, search};

    type ER = Dir<KeepHollow<Rel>>;

    fn hollowed() -> grw::MGraph<(), ER> {
        let mut g = mgraph![<(), ER>; N(0) & E().include(signs(1)) >> N(1)].unwrap();
        modify!(g, [X(0) & E().exclude::<Signs>() >> x(1)]).expect("the held part is excluded");
        g
    }

    #[test]
    fn modify_exclude_of_the_last_part_keeps_a_hollow_link() {
        let g = hollowed();
        let held: Vec<KeepHollow<Rel>> = g.edges_between(grw::id::N(0), grw::id::N(1)).map(|(_, v)| v.clone()).collect();
        assert_eq!(held.len(), 1);
        assert!(held[0].is_hollow());
        assert_eq!(g.edge_count(), 1);
    }

    #[test]
    fn a_plain_edge_matches_the_hollow_link() {
        let g = hollowed();
        let s = search![&g, get(Mono) { N(0) & E() >> N(1) }].unwrap();
        assert_eq!(s.iter().count(), 1);
    }

    #[test]
    fn has_does_not_match_the_hollow_link() {
        let g = hollowed();
        let s = search![&g, get(Mono) { N(0) & E().has::<Signs>() >> N(1) }].unwrap();
        assert_eq!(s.iter().count(), 0);
    }

    #[test]
    fn has_matches_once_the_link_is_refilled() {
        let mut g = hollowed();
        modify!(g, [X(0) & E().include(uses("svc")) >> x(1)]).expect("a hollow link takes a part");
        let s = search![&g, get(Mono) { N(0) & E().has::<Uses>().test(|u: &Uses| u.service == "svc") >> N(1) }].unwrap();
        assert_eq!(s.iter().count(), 1);
        let none = search![&g, get(Mono) { N(0) & E().has::<Signs>() >> N(1) }].unwrap();
        assert_eq!(none.iter().count(), 0);
    }

    #[test]
    fn the_same_kind_twice_is_a_compile_error_for_keep_hollow_too() {
        let g = hollowed();
        let err = search![&g, get(Mono) {
            N(0) & E().has::<Signs>() >> N(1),
            n(0) & E().has::<Signs>() >> n(1)
        }]
        .err()
        .expect("a pattern naming one kind twice is refused");
        match err {
            grw::search::error::Search::Edge(grw::search::error::Edge::PartRepeated { kind, .. }) => {
                assert_eq!(kind.of::<RelKind>(), Some(&RelKind::Signs));
            }
            other => panic!("expected PartRepeated, got {other:?}"),
        }
    }
}
