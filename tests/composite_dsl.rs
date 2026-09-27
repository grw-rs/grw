use grw::composite::{Composite, Part, PartKind, TypeSet, Typed};
use grw::graph::{edge, error, Graph, MGraph};
use grw::search::error as search_error;
use grw::search::Session;
use grw::{id, mgraph, modify, search};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signs {
    pub since: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uses {
    pub service: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Audits {
    pub year: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Part)]
pub enum Rel {
    Signs(Signs),
    Uses(Uses),
    Audits(Audits),
}

type Link = TypeSet<Rel>;
type Der = edge::Dir<Link>;
type Uer = edge::Undir<Link>;

fn signs(since: u32) -> Signs {
    Signs { since }
}

fn uses(service: u32) -> Uses {
    Uses { service }
}

fn link_between<ER: edge::Edge<Val = Link>, G: Graph<(), ER>>(g: &G, a: u32, b: u32) -> Option<Link> {
    g.edges_between(id::N(a), id::N(b)).map(|(_, v)| v.clone()).next()
}

fn kinds(link: &Link) -> Vec<RelKind> {
    link.parts().map(grw::composite::Kinded::kind).collect()
}

fn pairs<ER, G>(s: &Session<'_, (), ER, G>) -> Vec<(u32, u32)>
where
    ER: edge::Edge,
    ER::Val: Clone,
    G: Graph<(), ER>,
{
    let mut out: Vec<(u32, u32)> = s.iter().map(|m| ((*m.get(0u32).unwrap()), (*m.get(1u32).unwrap()))).collect();
    out.sort_unstable();
    out
}

fn board() -> MGraph<(), Der> {
    mgraph![<(), Der>;
        N(0) & E().include(signs(2019)) >> N(1),
        n(1) & E().include(signs(2023)).include(uses(7)) >> N(2),
        n(2) & E().include(uses(9)) >> N(3),
        n(0) & E().include(uses(4)) >> n(2),
        n(3) & E().include(Audits { year: 2024 }).include(signs(2021)) >> n(0),
    ]
    .unwrap()
}

mod graph_literal {
    use super::*;

    #[test]
    fn a_chain_of_includes_builds_one_link_holding_every_part() {
        let g = board();
        let link = link_between(&g, 1, 2).unwrap();
        assert_eq!(kinds(&link), vec![RelKind::Signs, RelKind::Uses]);
        assert_eq!(link.get::<Signs>(), Some(&signs(2023)));
        assert_eq!(link.get::<Uses>(), Some(&uses(7)));
        assert_eq!(g.edge_count(), 5);
    }

    #[test]
    fn a_whole_value_is_taken_as_given() {
        let whole = Link::from_part(Rel::Uses(uses(3)));
        let g = mgraph![<(), Der>; N(0) & E().val(whole.clone()) >> N(1)].unwrap();
        assert_eq!(link_between(&g, 0, 1), Some(whole));
    }

    #[test]
    fn an_undirected_literal_takes_parts_too() {
        let g = mgraph![<(), Uer>; N(0) & E().include(signs(1)).include(uses(2)) ^ N(1)].unwrap();
        assert_eq!(kinds(&link_between(&g, 1, 0).unwrap()), vec![RelKind::Signs, RelKind::Uses]);
    }

    #[test]
    fn a_kind_named_twice_is_a_typed_build_error() {
        let g = mgraph![<(), Der>; N(0) & E().include(signs(1)).include(uses(2)).include(signs(3)) >> N(1)];
        match g {
            Err(error::Build::Edge(error::Edge::Part { part: error::Part::Repeated(kind), .. })) => {
                assert_eq!(kind.of::<RelKind>(), Some(&RelKind::Signs));
            }
            Err(other) => panic!("expected a repeated part, got {other:?}"),
            Ok(_) => panic!("expected a repeated part, the graph was built"),
        }
    }

    #[test]
    fn an_equal_part_named_twice_is_still_a_typed_build_error() {
        let g = mgraph![<(), Der>; N(0) & E().include(signs(1)).include(signs(1)) >> N(1)];
        assert!(matches!(g, Err(error::Build::Edge(error::Edge::Part { part: error::Part::Repeated(_), .. }))));
    }

    #[test]
    fn a_vgraph_literal_refuses_the_same_way() {
        let g = grw::vgraph![<(), Der>; N(0) & E().include(uses(1)).include(uses(2)) >> N(1)];
        assert!(matches!(g, Err(error::Build::Edge(error::Edge::Part { part: error::Part::Repeated(_), .. }))));
    }
}

mod modify_ops {
    use super::*;

    #[test]
    fn include_creates_the_link_when_the_pair_is_vacant() {
        let mut g = board();
        modify!(g, [X(1) & E().include(uses(5)) >> x(3)]).unwrap();
        assert_eq!(kinds(&link_between(&g, 1, 3).unwrap()), vec![RelKind::Uses]);
    }

    #[test]
    fn include_adds_a_part_to_a_held_link() {
        let mut g = board();
        modify!(g, [X(0) & E().include(Audits { year: 1 }) >> x(1)]).unwrap();
        assert_eq!(kinds(&link_between(&g, 0, 1).unwrap()), vec![RelKind::Signs, RelKind::Audits]);
    }

    #[test]
    fn include_of_a_different_part_of_a_held_kind_is_refused() {
        let mut g = board();
        let err = modify!(g, [X(0) & E().include(signs(1999)) >> x(1)]).err().expect("the op is refused");
        match err {
            grw::modify::error::Modify::Apply(grw::modify::error::Apply::Edge(
                grw::modify::error::apply::Edge::PartRejected { refused, .. },
            )) => {
                assert_eq!(refused.of::<Link>().map(|r| r.kind), Some(RelKind::Signs));
            }
            other => panic!("expected PartRejected, got {other:?}"),
        }
    }

    #[test]
    fn include_over_an_undirected_pending_edge() {
        let mut g = mgraph![<(), Uer>; N(0) & E().include(signs(1)) ^ N(1)].unwrap();
        modify!(g, [X(1) & E().include(uses(2)) ^ x(0)]).unwrap();
        assert_eq!(kinds(&link_between(&g, 0, 1).unwrap()), vec![RelKind::Signs, RelKind::Uses]);
    }

    #[test]
    fn exclude_by_type_leaves_the_other_parts() {
        let mut g = board();
        modify!(g, [X(1) & E().exclude::<Signs>() >> x(2)]).unwrap();
        assert_eq!(kinds(&link_between(&g, 1, 2).unwrap()), vec![RelKind::Uses]);
    }

    #[test]
    fn exclude_of_the_last_part_deletes_a_type_set_link() {
        let mut g = board();
        modify!(g, [X(0) & E().exclude::<Signs>() >> x(1)]).unwrap();
        assert_eq!(link_between(&g, 0, 1), None);
        assert_eq!(g.edge_count(), 4);
    }

    #[test]
    fn exclude_by_runtime_kind() {
        let mut g = board();
        modify!(g, [X(1) & E().exclude_kind(RelKind::Uses) >> x(2)]).unwrap();
        assert_eq!(kinds(&link_between(&g, 1, 2).unwrap()), vec![RelKind::Signs]);
    }

    #[test]
    fn exclude_of_a_kind_not_held_is_refused() {
        let mut g = board();
        let err = modify!(g, [X(0) & E().exclude::<Uses>() >> x(1)]).err().expect("the op is refused");
        match err {
            grw::modify::error::Modify::Apply(grw::modify::error::Apply::Edge(
                grw::modify::error::apply::Edge::PartNotHeld { refused, .. },
            )) => {
                assert_eq!(refused.of::<Link>().map(|r| r.kind), Some(RelKind::Uses));
            }
            other => panic!("expected PartNotHeld, got {other:?}"),
        }
    }

    fn untouched(m: &grw::modify::Modification<(), Der>) -> bool {
        m.added_edges.is_empty()
            && m.removed_edges.is_empty()
            && m.swapped_edge_vals.is_empty()
            && m.removed_nodes.is_empty()
            && m.swapped_node_vals.is_empty()
    }

    #[test]
    fn include_of_an_equal_part_writes_nothing() {
        let mut g = board();
        let m = modify!(g, [X(0) & E().include(signs(2019)) >> x(1)]).unwrap();
        assert!(untouched(&m));
        assert_eq!(kinds(&link_between(&g, 0, 1).unwrap()), vec![RelKind::Signs]);
    }

    #[test]
    fn a_net_equal_batch_writes_nothing() {
        let mut g = board();
        let m = modify!(g, [X(0) & E().include(uses(1)) >> x(1), x(0) & E().exclude::<Uses>() >> x(1)]).unwrap();
        assert!(untouched(&m));
        assert_eq!(link_between(&g, 0, 1), Some(Link::from_part(Rel::Signs(signs(2019)))));
    }

    #[test]
    fn include_on_a_descending_pair_joins_the_held_link() {
        let mut g = board();
        let m = modify!(g, [X(3) & E().include(uses(1)) >> x(0)]).unwrap();
        assert_eq!(m.swapped_edge_vals.len(), 1);
        assert!(m.added_edges.is_empty());
        assert_eq!(g.edge_count(), 5);
        assert_eq!(kinds(&link_between(&g, 3, 0).unwrap()), vec![RelKind::Signs, RelKind::Uses, RelKind::Audits]);
    }

    #[test]
    fn deleting_the_whole_link_drops_every_part() {
        let mut g = board();
        modify!(g, [X(1) & !e() >> x(2)]).unwrap();
        assert_eq!(link_between(&g, 1, 2), None);
    }
}

mod search_parts {
    use super::*;

    #[test]
    fn has_matches_every_link_holding_the_part_among_others() {
        let g = board();
        let s = search![&g, get(Mono) { N(0) & E().has::<Signs>() >> N(1) }].unwrap();
        assert_eq!(pairs(&s), vec![(0, 1), (1, 2), (3, 0)]);
    }

    #[test]
    fn has_chained_is_a_conjunction() {
        let g = board();
        let s = search![&g, get(Mono) { N(0) & E().has::<Signs>().has::<Uses>() >> N(1) }].unwrap();
        assert_eq!(pairs(&s), vec![(1, 2)]);
    }

    #[test]
    fn test_after_has_reads_the_focused_part() {
        let g = board();
        let s = search![&g, get(Mono) { N(0) & E().has::<Signs>().test(|s: &Signs| s.since > 2020) >> N(1) }].unwrap();
        assert_eq!(pairs(&s), vec![(1, 2), (3, 0)]);
    }

    #[test]
    fn each_test_reads_the_has_just_before_it() {
        let g = board();
        let s = search![&g, get(Mono) {
            N(0) & E().has::<Signs>().test(|s: &Signs| s.since > 2020).has::<Uses>().test(|u: &Uses| u.service == 7) >> N(1)
        }]
        .unwrap();
        assert_eq!(pairs(&s), vec![(1, 2)]);
    }

    #[test]
    fn test_without_has_reads_the_whole_value() {
        let g = board();
        let s = search![&g, get(Mono) { N(0) & E().test(|v: &Link| v.parts().count() == 2) >> N(1) }].unwrap();
        assert_eq!(pairs(&s), vec![(1, 2), (3, 0)]);
    }

    #[test]
    fn plain_edge_still_matches_every_link() {
        let g = board();
        let s = search![&g, get(Mono) { N(0) & E() >> N(1) }].unwrap();
        assert_eq!(pairs(&s), vec![(0, 1), (0, 2), (1, 2), (2, 3), (3, 0)]);
    }

    #[test]
    fn has_kind_takes_a_runtime_kind_and_focuses_the_raw_part() {
        let g = board();
        let s = search![&g, get(Mono) {
            N(0) & E().has_kind(RelKind::Signs).test(|p: &Rel| matches!(p, Rel::Signs(s) if s.since < 2022)) >> N(1)
        }]
        .unwrap();
        assert_eq!(pairs(&s), vec![(0, 1), (3, 0)]);
    }

    #[test]
    fn undirected_has() {
        let g = mgraph![<(), Uer>;
            N(0) & E().include(signs(1)) ^ N(1),
            n(1) & E().include(uses(1)) ^ N(2),
        ]
        .unwrap();
        let s = search![&g, get(Mono) { N(0) & E().has::<Uses>() ^ N(1) }].unwrap();
        assert_eq!(pairs(&s), vec![(1, 2), (2, 1)]);
    }

    #[test]
    fn negated_has_rejects_only_links_holding_that_part() {
        let g = board();
        let s = search![&g, get(Mono) {
            N(0) & E().has::<Uses>() >> N(1),
            N(2) & E().has::<Signs>() >> n(0),
            n(2) & !E().has::<Signs>() >> n(1)
        }]
        .unwrap();
        let mut got: Vec<(u32, u32, u32)> = s
            .iter()
            .map(|m| ((*m.get(0u32).unwrap()), (*m.get(1u32).unwrap()), (*m.get(2u32).unwrap())))
            .collect();
        got.sort_unstable();
        assert_eq!(got, vec![(0, 2, 3), (1, 2, 0), (2, 3, 1)]);
    }

    #[test]
    fn negated_plain_edge_rejects_any_link() {
        let g = board();
        let s = search![&g, get(Mono) {
            N(0) & E().has::<Uses>() >> N(1),
            N(2) & E().has::<Signs>() >> n(0),
            n(2) & !E() >> n(1)
        }]
        .unwrap();
        let mut got: Vec<(u32, u32, u32)> = s
            .iter()
            .map(|m| ((*m.get(0u32).unwrap()), (*m.get(1u32).unwrap()), (*m.get(2u32).unwrap())))
            .collect();
        got.sort_unstable();
        assert_eq!(got, vec![(0, 2, 3), (2, 3, 1)]);
    }
}

mod merge {
    use super::*;

    #[test]
    fn two_terms_on_one_pair_merge_into_one_slot_constraint() {
        let g = board();
        let s = search![&g, get(Mono) {
            N(0) & E().has::<Signs>() >> N(1),
            n(0) & E().has::<Uses>() >> n(1)
        }]
        .unwrap();
        assert_eq!(s.query().edge_count(), 1);
        assert_eq!(pairs(&s), vec![(1, 2)]);
    }

    #[test]
    fn a_term_written_from_the_other_end_merges_too() {
        let g = board();
        let s = search![&g, get(Mono) {
            N(0) & E().has::<Signs>() >> N(1),
            n(1) & E().has::<Uses>() << n(0)
        }]
        .unwrap();
        assert_eq!(s.query().edge_count(), 1);
        assert_eq!(pairs(&s), vec![(1, 2)]);
    }

    #[test]
    fn has_with_negated_has_on_one_pair_holds_one_and_not_the_other() {
        let g = board();
        let s = search![&g, get(Mono) {
            N(0) & E().has::<Signs>() >> N(1),
            n(0) & !E().has::<Uses>() >> n(1)
        }]
        .unwrap();
        assert_eq!(s.query().edge_count(), 1);
        assert_eq!(pairs(&s), vec![(0, 1), (3, 0)]);
    }

    #[test]
    fn negated_has_written_first_still_folds_into_the_positive_term() {
        let g = board();
        let s = search![&g, get(Mono) {
            N(0) & !E().has::<Uses>() >> N(1),
            n(0) & E().has::<Signs>() >> n(1)
        }]
        .unwrap();
        assert_eq!(s.query().edge_count(), 1);
        assert_eq!(pairs(&s), vec![(0, 1), (3, 0)]);
    }

    #[test]
    fn a_plain_positive_term_yields_to_the_has_term() {
        let g = board();
        let s = search![&g, get(Mono) {
            N(0) & E() >> N(1),
            n(0) & E().has::<Uses>() >> n(1)
        }]
        .unwrap();
        assert_eq!(pairs(&s), vec![(0, 2), (1, 2), (2, 3)]);
    }

    #[test]
    fn a_predicate_merges_with_its_own_part() {
        let g = board();
        let s = search![&g, get(Mono) {
            N(0) & E().has::<Signs>().test(|s: &Signs| s.since > 2020) >> N(1),
            n(0) & !E().has::<Audits>() >> n(1)
        }]
        .unwrap();
        assert_eq!(pairs(&s), vec![(1, 2)]);
    }

    #[test]
    fn the_same_kind_twice_in_one_chain_is_a_compile_error() {
        let g = board();
        let err = search![&g, get(Mono) { N(0) & E().has::<Signs>().has::<Signs>() >> N(1) }].err().unwrap();
        repeated_signs(err);
    }

    #[test]
    fn the_same_kind_twice_across_terms_is_a_compile_error() {
        let g = board();
        let err = search![&g, get(Mono) {
            N(0) & E().has::<Signs>().test(|s: &Signs| s.since > 1) >> N(1),
            n(0) & E().has::<Signs>().test(|s: &Signs| s.since < 3000) >> n(1)
        }]
        .err()
        .unwrap();
        repeated_signs(err);
    }

    #[test]
    fn the_same_kind_twice_under_negation_is_a_compile_error() {
        let g = board();
        let err = search![&g, get(Mono) {
            N(0) & E() >> N(1),
            n(0) & !E().has::<Uses>().has::<Uses>() >> n(1)
        }]
        .err()
        .unwrap();
        assert!(matches!(err, search_error::Search::Edge(search_error::Edge::PartRepeated { .. })));
    }

    fn repeated_signs(err: search_error::Search) {
        match err {
            search_error::Search::Edge(search_error::Edge::PartRepeated { kind, .. }) => {
                let kind: &PartKind = &kind;
                assert_eq!(kind.of::<RelKind>(), Some(&RelKind::Signs));
            }
            other => panic!("expected PartRepeated, got {other:?}"),
        }
    }

    #[test]
    fn no_link_with_a_positive_has_is_a_contradiction() {
        let g = board();
        let err = search![&g, get(Mono) {
            N(0) & E().has::<Signs>() >> N(1),
            n(0) & !E() >> n(1)
        }]
        .err()
        .unwrap();
        assert!(matches!(err, search_error::Search::Edge(search_error::Edge::Contradictory { .. })));
    }

    #[test]
    fn requiring_and_forbidding_one_kind_is_a_contradiction() {
        let g = board();
        let err = search![&g, get(Mono) {
            N(0) & E().has::<Signs>() >> N(1),
            n(0) & !E().has::<Signs>() >> n(1)
        }]
        .err()
        .unwrap();
        assert!(matches!(err, search_error::Search::Edge(search_error::Edge::Contradictory { .. })));
    }

    #[test]
    fn forbidding_every_part_the_link_requires_is_a_contradiction() {
        let g = board();
        let err = search![&g, get(Mono) {
            N(0) & E().has::<Signs>().has::<Uses>() >> N(1),
            n(0) & !E().has::<Uses>().has::<Signs>() >> n(1)
        }]
        .err()
        .unwrap();
        assert!(matches!(err, search_error::Search::Edge(search_error::Edge::Contradictory { .. })));
    }

    #[test]
    fn forbidding_every_part_the_link_requires_under_a_test_is_a_contradiction() {
        let g = board();
        let err = search![&g, get(Mono) {
            N(0) & E().has::<Signs>().test(|s: &Signs| s.since > 2020).has::<Uses>() >> N(1),
            n(0) & !E().has::<Signs>().has::<Uses>() >> n(1)
        }]
        .err()
        .unwrap();
        assert!(matches!(err, search_error::Search::Edge(search_error::Edge::Contradictory { .. })));
    }

    #[test]
    fn forbidding_a_required_part_beside_one_not_required_is_allowed() {
        let g = board();
        let s = search![&g, get(Mono) {
            N(0) & E().has::<Signs>() >> N(1),
            n(0) & !E().has::<Signs>().has::<Audits>() >> n(1)
        }]
        .unwrap();
        assert_eq!(s.query().edge_count(), 1);
        assert_eq!(pairs(&s), vec![(0, 1), (1, 2)]);
    }

    #[test]
    fn a_ban_cluster_has_on_the_get_pair_is_a_conflicting_predicate() {
        let g = board();
        let err = search![&g,
            get(Mono) { N(0) & E().has::<Signs>() >> N(1) },
            ban(Mono) { n(0) & E().has::<Uses>() >> n(1) }
        ]
        .err()
        .unwrap();
        assert!(matches!(err, search_error::Search::Edge(search_error::Edge::ConflictingPred { src: 0, tgt: 1 })));
    }

    #[test]
    fn a_ban_cluster_negated_has_on_the_get_pair_leaves_the_get_term_unchanged() {
        let g = board();
        let s = search![&g,
            get(Mono) { N(0) & E().has::<Signs>() >> N(1) },
            ban(Mono) { n(0) & !E().has::<Uses>() >> n(1) }
        ]
        .unwrap();
        assert_eq!(s.query().edge_count(), 2);
        assert_eq!(pairs(&s), vec![(1, 2)]);
    }

    #[test]
    fn forbidding_a_tested_part_the_link_requires_is_allowed() {
        let g = board();
        let s = search![&g, get(Mono) {
            N(0) & E().has::<Signs>() >> N(1),
            n(0) & !E().has::<Signs>().test(|s: &Signs| s.since > 2022) >> n(1)
        }]
        .unwrap();
        assert_eq!(pairs(&s), vec![(0, 1), (3, 0)]);
    }
}

mod readme {
    use super::*;

    #[test]
    fn the_readme_composite_walkthrough_runs_as_written() {
        let mut g = mgraph![<(), Der>;
            N(0) & E().include(Signs { since: 2023 }).include(Uses { service: 7 }) >> N(1),
            n(1) & E().include(Uses { service: 9 }) >> N(2),
        ]
        .unwrap();

        modify!(g, [X(1) & E().include(Signs { since: 2024 }) >> x(2)]).unwrap();
        modify!(g, [X(0) & E().exclude::<Uses>() >> x(1)]).unwrap();
        modify!(g, [X(1) & E().exclude_kind(RelKind::Uses) >> x(2)]).unwrap();
        modify!(g, [X(0) & !e() >> x(1)]).unwrap();

        let s = search![&g, get(Mono) {
            N(0) & E().has::<Signs>().test(|s: &Signs| s.since > 2020) >> N(1)
        }]
        .unwrap();
        assert_eq!(pairs(&s), vec![(1, 2)]);
        assert_eq!(kinds(&link_between(&g, 1, 2).unwrap()), vec![RelKind::Signs]);
    }
}

mod anydir {
    use super::*;
    use grw::composite::{KindPresent, NotHeld};
    use grw::graph::edge::{anydir, AnyVal};
    use grw::graph::VGraph;
    use grw::modify::error::{apply, Apply, Modify};
    use grw::modify::{node, Modification, Node};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Knows {
        pub since: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Met {
        pub at: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Part)]
    pub enum Tie {
        Knows(Knows),
        Met(Met),
    }

    type Bond = TypeSet<Tie>;
    type Aer = edge::Anydir<Bond, Link>;
    type Ops = Vec<Node<(), Aer>>;

    fn knows(since: u32) -> Knows {
        Knows { since }
    }

    fn bond(parts: Vec<Tie>) -> Bond {
        let mut parts = parts.into_iter();
        let first = Bond::from_part(parts.next().expect("a bond holds a part"));
        parts.fold(first, |held, part| match held.with_part(part).unwrap() {
            grw::composite::Included::Added(next) => next,
            grw::composite::Included::Held => held,
        })
    }

    fn link(parts: Vec<Rel>) -> Link {
        let mut parts = parts.into_iter();
        let first = Link::from_part(parts.next().expect("a link holds a part"));
        parts.fold(first, |held, part| match held.with_part(part).unwrap() {
            grw::composite::Included::Added(next) => next,
            grw::composite::Included::Held => held,
        })
    }

    trait Subject: Graph<(), Aer> + Sized {
        fn board() -> Self;
        fn applied(self, ops: Ops) -> Result<(Self, Modification<(), Aer>), Modify>;
    }

    impl Subject for MGraph<(), Aer> {
        fn board() -> Self {
            mgraph![<(), Aer>;
                N(0) & E().include(knows(2019)) ^ N(1),
                n(0) & E().include(signs(2020)) >> n(1),
                N(2) & E().include(knows(2021)).include(Met { at: 4 }) ^ n(1),
                n(2) & E().include(uses(7)) >> n(1),
                N(3),
            ]
            .unwrap()
        }

        fn applied(mut self, ops: Ops) -> Result<(Self, Modification<(), Aer>), Modify> {
            let modification = self.modify(ops)?;
            Ok((self, modification))
        }
    }

    impl Subject for VGraph<(), Aer> {
        fn board() -> Self {
            grw::vgraph![<(), Aer>;
                N(0) & E().include(knows(2019)) ^ N(1),
                n(0) & E().include(signs(2020)) >> n(1),
                N(2) & E().include(knows(2021)).include(Met { at: 4 }) ^ n(1),
                n(2) & E().include(uses(7)) >> n(1),
                N(3),
            ]
            .unwrap()
        }

        fn applied(self, ops: Ops) -> Result<(Self, Modification<(), Aer>), Modify> {
            self.modify(ops)
        }
    }

    fn undir<G: Subject>(g: &G, a: u32, b: u32) -> Option<&AnyVal<Bond, Link>> {
        g.get_edge_val(anydir::E::U(a, b))
    }

    fn dir<G: Subject>(g: &G, a: u32, b: u32) -> Option<&AnyVal<Bond, Link>> {
        g.get_edge_val(anydir::E::D(a, b))
    }

    fn untouched(m: &Modification<(), Aer>) -> bool {
        m.added_edges.is_empty()
            && m.removed_edges.is_empty()
            && m.swapped_edge_vals.is_empty()
            && m.removed_nodes.is_empty()
            && m.swapped_node_vals.is_empty()
    }

    fn the_literal_puts_each_part_in_its_slot_class<G: Subject>() {
        let g = G::board();
        assert_eq!(undir(&g, 0, 1), Some(&AnyVal::Undir(bond(vec![Tie::Knows(knows(2019))]))));
        assert_eq!(dir(&g, 0, 1), Some(&AnyVal::Dir(link(vec![Rel::Signs(signs(2020))]))));
        assert_eq!(undir(&g, 1, 2), Some(&AnyVal::Undir(bond(vec![Tie::Knows(knows(2021)), Tie::Met(Met { at: 4 })]))));
        assert_eq!(g.edge_count(), 4);
    }

    fn include_on_und_lands_in_the_undirected_class<G: Subject>() {
        let (g, m) = G::board().applied(modify![x(3) & E().include(knows(2024)) ^ x(0)]).unwrap();
        assert_eq!(m.added_edges.len(), 1);
        assert_eq!(undir(&g, 0, 3), Some(&AnyVal::Undir(bond(vec![Tie::Knows(knows(2024))]))));
        assert_eq!(dir(&g, 3, 0), None);
        assert_eq!(dir(&g, 0, 3), None);
    }

    fn include_on_shr_and_shl_lands_in_the_directed_class<G: Subject>() {
        let (g, m) = G::board()
            .applied(modify![x(3) & E().include(signs(2024)) >> x(0), x(3) & E().include(uses(5)) << x(2)])
            .unwrap();
        assert_eq!(m.added_edges.len(), 2);
        assert_eq!(dir(&g, 3, 0), Some(&AnyVal::Dir(link(vec![Rel::Signs(signs(2024))]))));
        assert_eq!(dir(&g, 2, 3), Some(&AnyVal::Dir(link(vec![Rel::Uses(uses(5))]))));
        assert_eq!(undir(&g, 0, 3), None);
        assert_eq!(undir(&g, 2, 3), None);
    }

    fn include_joins_the_held_link_of_its_own_class_only<G: Subject>() {
        let (g, m) = G::board().applied(modify![x(0) & E().include(Met { at: 9 }) ^ x(1)]).unwrap();
        assert_eq!(m.swapped_edge_vals.len(), 1);
        assert!(m.added_edges.is_empty());
        assert_eq!(undir(&g, 0, 1), Some(&AnyVal::Undir(bond(vec![Tie::Knows(knows(2019)), Tie::Met(Met { at: 9 })]))));
        assert_eq!(dir(&g, 0, 1), Some(&AnyVal::Dir(link(vec![Rel::Signs(signs(2020))]))));

        let (g, _) = g.applied(modify![x(0) & E().include(uses(3)) >> x(1)]).unwrap();
        assert_eq!(dir(&g, 0, 1), Some(&AnyVal::Dir(link(vec![Rel::Signs(signs(2020)), Rel::Uses(uses(3))]))));
        assert_eq!(undir(&g, 0, 1), Some(&AnyVal::Undir(bond(vec![Tie::Knows(knows(2019)), Tie::Met(Met { at: 9 })]))));
    }

    fn include_of_an_equal_part_writes_nothing<G: Subject>() {
        let (g, m) = G::board()
            .applied(modify![x(0) & E().include(knows(2019)) ^ x(1), x(0) & E().include(signs(2020)) >> x(1)])
            .unwrap();
        assert!(untouched(&m));
        assert_eq!(g.edge_count(), 4);
    }

    fn include_of_a_different_part_of_a_held_kind_is_refused_by_the_slot_value<G: Subject>() {
        let err = G::board().applied(modify![x(1) & E().include(knows(2000)) ^ x(0)]).err().expect("refused");
        let Modify::Apply(Apply::Edge(apply::Edge::PartRejected { a, b, refused })) = err else {
            panic!("expected PartRejected, got {err:?}")
        };
        assert_eq!((a, b), (id::N(1), id::N(0)));
        assert_eq!(refused.of::<Bond>(), Some(&KindPresent { kind: TieKind::Knows }));

        let err = G::board().applied(modify![x(0) & E().include(signs(1999)) >> x(1)]).err().expect("refused");
        let Modify::Apply(Apply::Edge(apply::Edge::PartRejected { refused, .. })) = err else {
            panic!("expected PartRejected, got {err:?}")
        };
        assert_eq!(refused.of::<Link>(), Some(&KindPresent { kind: RelKind::Signs }));
    }

    fn exclude_to_vacant_deletes_only_that_class<G: Subject>() {
        let (g, m) = G::board().applied(modify![x(0) & E().exclude::<Knows>() ^ x(1)]).unwrap();
        assert_eq!(m.removed_edges.len(), 1);
        assert_eq!(m.removed_edges[0].3, AnyVal::Undir(bond(vec![Tie::Knows(knows(2019))])));
        assert_eq!(undir(&g, 0, 1), None);
        assert_eq!(dir(&g, 0, 1), Some(&AnyVal::Dir(link(vec![Rel::Signs(signs(2020))]))));
        assert_eq!(g.edge_count(), 3);

        let (g, _) = g.applied(modify![x(0) & E().exclude_kind(RelKind::Signs) >> x(1)]).unwrap();
        assert_eq!(dir(&g, 0, 1), None);
        assert_eq!(g.edge_count(), 2);
    }

    fn exclude_by_runtime_kind_leaves_the_rest<G: Subject>() {
        let (g, m) = G::board().applied(modify![x(1) & E().exclude_kind(TieKind::Met) ^ x(2)]).unwrap();
        assert_eq!(m.swapped_edge_vals.len(), 1);
        assert_eq!(undir(&g, 1, 2), Some(&AnyVal::Undir(bond(vec![Tie::Knows(knows(2021))]))));
    }

    fn exclude_of_a_kind_not_held_is_refused_by_the_slot_value<G: Subject>() {
        let err = G::board().applied(modify![x(0) & E().exclude::<Met>() ^ x(1)]).err().expect("refused");
        let Modify::Apply(Apply::Edge(apply::Edge::PartNotHeld { refused, .. })) = err else {
            panic!("expected PartNotHeld, got {err:?}")
        };
        assert_eq!(refused.of::<Bond>(), Some(&NotHeld { kind: TieKind::Met }));

        let err = G::board().applied(modify![x(0) & E().exclude::<Uses>() >> x(1)]).err().expect("refused");
        let Modify::Apply(Apply::Edge(apply::Edge::PartNotHeld { refused, .. })) = err else {
            panic!("expected PartNotHeld, got {err:?}")
        };
        assert_eq!(refused.of::<Link>(), Some(&NotHeld { kind: RelKind::Uses }));
    }

    fn exclude_on_a_vacant_slot_of_a_linked_pair_is_the_missing_edge<G: Subject>() {
        let err = G::board().applied(modify![x(1) & E().exclude::<Signs>() >> x(0)]).err().expect("refused");
        assert!(matches!(err, Modify::Apply(Apply::Edge(apply::Edge::NotFound(a, b))) if (a, b) == (id::N(1), id::N(0))));
    }

    fn a_net_equal_batch_writes_nothing<G: Subject>() {
        let (g, m) = G::board()
            .applied(modify![
                x(0) & E().include(Met { at: 1 }) ^ x(1),
                x(1) & E().exclude::<Met>() ^ x(0),
                x(0) & E().include(uses(2)) >> x(1),
                x(0) & E().exclude_kind(RelKind::Uses) >> x(1),
            ])
            .unwrap();
        assert!(untouched(&m));
        assert_eq!(undir(&g, 0, 1), Some(&AnyVal::Undir(bond(vec![Tie::Knows(knows(2019))]))));
        assert_eq!(dir(&g, 0, 1), Some(&AnyVal::Dir(link(vec![Rel::Signs(signs(2020))]))));
    }

    fn part_ops_on_a_descending_pair_find_the_held_link<G: Subject>() {
        let (g, m) = G::board()
            .applied(modify![x(2) & E().include(signs(2022)) >> x(1), x(2) & E().exclude::<Knows>() ^ x(1)])
            .unwrap();
        assert_eq!(m.swapped_edge_vals.len(), 2);
        assert!(m.added_edges.is_empty());
        assert_eq!(dir(&g, 2, 1), Some(&AnyVal::Dir(link(vec![Rel::Signs(signs(2022)), Rel::Uses(uses(7))]))));
        assert_eq!(undir(&g, 2, 1), Some(&AnyVal::Undir(bond(vec![Tie::Met(Met { at: 4 })]))));
        assert_eq!(g.edge_count(), 4);
    }

    fn off_class(slot: anydir::Slot, part: Ops) -> Ops {
        let planted = Node::Exist(node::Exist::Bind {
            id: id::N(3),
            op: node::Bind::Ref,
            edges: vec![grw::modify::edge::Edge::New {
                slot,
                val: match slot {
                    anydir::UND => AnyVal::Dir(link(vec![Rel::Signs(signs(1))])),
                    _ => AnyVal::Undir(bond(vec![Tie::Knows(knows(1))])),
                },
                target: Node::Exist(node::Exist::Bind { id: id::N(0), op: node::Bind::Ref, edges: vec![] }),
            }],
        });
        vec![planted].into_iter().chain(part).collect()
    }

    fn a_held_value_of_the_other_class_is_a_typed_refusal<G: Subject>() {
        let (g, _) = G::board().applied(off_class(anydir::UND, vec![])).unwrap();
        let err = g.applied(modify![x(3) & E().include(knows(5)) ^ x(0)]).err().expect("refused");
        assert!(matches!(
            err,
            Modify::Apply(Apply::Edge(apply::Edge::SlotClass(apply::SlotClass { a, b, expected: apply::SlotClassKind::Undir })))
                if (a, b) == (id::N(3), id::N(0))
        ));

        let (g, _) = G::board().applied(off_class(anydir::SRC, vec![])).unwrap();
        let err = g.applied(modify![x(3) & E().exclude::<Signs>() >> x(0)]).err().expect("refused");
        assert_eq!(err.to_string(), "the part op on N(3)-N(0) needs a directed link");
        assert!(matches!(
            err,
            Modify::Apply(Apply::Edge(apply::Edge::SlotClass(apply::SlotClass { a, b, expected: apply::SlotClassKind::Dir })))
                if (a, b) == (id::N(3), id::N(0))
        ));
    }

    macro_rules! on_both_graphs {
        ($($case:ident),* $(,)?) => {
            mod mgraph {
                $(#[test] fn $case() { super::$case::<super::MGraph<(), super::Aer>>() })*
            }
            mod vgraph {
                $(#[test] fn $case() { super::$case::<super::VGraph<(), super::Aer>>() })*
            }
        };
    }

    on_both_graphs!(
        the_literal_puts_each_part_in_its_slot_class,
        include_on_und_lands_in_the_undirected_class,
        include_on_shr_and_shl_lands_in_the_directed_class,
        include_joins_the_held_link_of_its_own_class_only,
        include_of_an_equal_part_writes_nothing,
        include_of_a_different_part_of_a_held_kind_is_refused_by_the_slot_value,
        exclude_to_vacant_deletes_only_that_class,
        exclude_by_runtime_kind_leaves_the_rest,
        exclude_of_a_kind_not_held_is_refused_by_the_slot_value,
        exclude_on_a_vacant_slot_of_a_linked_pair_is_the_missing_edge,
        a_net_equal_batch_writes_nothing,
        part_ops_on_a_descending_pair_find_the_held_link,
        a_held_value_of_the_other_class_is_a_typed_refusal,
    );

    #[test]
    fn has_over_und_matches_only_undirected_links() {
        let g = <MGraph<(), Aer> as Subject>::board();
        let s = search![&g, get(Mono) { N(0) & E().has::<Knows>() ^ N(1) }].unwrap();
        assert_eq!(pairs(&s), vec![(0, 1), (1, 0), (1, 2), (2, 1)]);
        let s = search![&g, get(Mono) { N(0) & E().has::<Met>().test(|m: &Met| m.at == 4) ^ N(1) }].unwrap();
        assert_eq!(pairs(&s), vec![(1, 2), (2, 1)]);
        let s = search![&g, get(Mono) { N(0) & E().has_kind(TieKind::Knows) ^ N(1), n(0) & !E().has::<Met>() ^ n(1) }]
            .unwrap();
        assert_eq!(pairs(&s), vec![(0, 1), (1, 0)]);
    }

    #[test]
    fn has_over_shr_matches_only_directed_links() {
        let g = <MGraph<(), Aer> as Subject>::board();
        let s = search![&g, get(Mono) { N(0) & E().has::<Signs>() >> N(1) }].unwrap();
        assert_eq!(pairs(&s), vec![(0, 1)]);
        let s = search![&g, get(Mono) { N(0) & E().has_kind(RelKind::Uses) << N(1) }].unwrap();
        assert_eq!(pairs(&s), vec![(1, 2)]);
        let s = search![&g, get(Mono) { N(0) & E().has::<Signs>() >> N(1), n(0) & E().has::<Knows>() ^ n(1) }].unwrap();
        assert_eq!(pairs(&s), vec![(0, 1)]);
    }
}
