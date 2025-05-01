pub mod dsl;
pub mod engine;
pub mod error;
#[doc(hidden)]
pub mod query;

pub use query::compile;
pub use query::Query;
pub use query::Search;
pub use query::Resolved;
pub use query::Unresolved;
pub use query::Bound;
pub use query::BindError;
pub use engine::{Seq, Par, Graph, Session, Indexed, Tier, Raw, Rev, RevCsr, RevCsrVal, Match, MatchedEdge, TranslatedMatch};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Morphism {
    Iso,
    SubIso,
    EpiMono,
    Mono,
    Epi,
    Homo,
}

impl Morphism {
    pub fn is_injective(self) -> bool {
        matches!(self, Morphism::Iso | Morphism::SubIso | Morphism::EpiMono | Morphism::Mono)
    }

    pub fn is_surjective(self) -> bool {
        matches!(self, Morphism::Iso | Morphism::EpiMono | Morphism::Epi)
    }

    pub fn is_induced(self) -> bool {
        matches!(self, Morphism::Iso | Morphism::SubIso)
    }

    pub fn meet(self, other: Morphism) -> Morphism {
        use Morphism::*;
        if self == other { return self; }
        match (self, other) {
            (Iso, _) | (_, Iso) => Iso,
            (SubIso, Epi) | (Epi, SubIso) => Iso,
            (SubIso, EpiMono) | (EpiMono, SubIso) => Iso,
            (SubIso, _) | (_, SubIso) => SubIso,
            (Mono, Epi) | (Epi, Mono) => EpiMono,
            (EpiMono, Mono) | (Mono, EpiMono) => EpiMono,
            (EpiMono, Epi) | (Epi, EpiMono) => EpiMono,
            (EpiMono, Homo) | (Homo, EpiMono) => EpiMono,
            (Mono, Homo) | (Homo, Mono) => Mono,
            (Epi, Homo) | (Homo, Epi) => Epi,
            (Homo, Homo) => Homo,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Get,
    Ban,
}

#[doc(hidden)]
#[macro_export]
macro_rules! __search_clusters {
    (@acc [$($clusters:expr),*]) => {
        vec![$($clusters),*]
    };
    (@acc [$($clusters:expr),*] get($m:expr) { $($expr:expr),* $(,)? } $(, $($rest:tt)*)?) => {
        $crate::__search_clusters!(
            @acc [$($clusters,)* $crate::search::dsl::get($m, vec![$($expr.into()),*])]
            $($($rest)*)?
        )
    };
    (@acc [$($clusters:expr),*] ban($m:expr) { $($expr:expr),* $(,)? } $(, $($rest:tt)*)?) => {
        $crate::__search_clusters!(
            @acc [$($clusters,)* $crate::search::dsl::ban($m, vec![$($expr.into()),*])]
            $($($rest)*)?
        )
    };
}

#[macro_export]
macro_rules! search {
    [<$nv:ty, $er:ty>; $($body:tt)*] => {{
        #[allow(unused_imports)]
        use $crate::search::dsl::*;
        let clusters: Vec<$crate::search::dsl::ClusterOps<$nv, $er>> =
            $crate::__search_clusters!(@acc [] $($body)*);
        $crate::search::query::compile(clusters)
    }};
    [graph ! [$($g:tt)*], $($body:tt)*] => {{
        #[allow(unused_imports)]
        use $crate::search::dsl::*;
        $crate::graph![$($g)*]
            .map_err(|e| $crate::search::error::Search::GraphBuild(Box::new(e)))
            .and_then(|__g| {
                let clusters = $crate::__search_clusters!(@acc [] $($body)*);
                $crate::search::query::compile(clusters).and_then(|compiled| {
                    $crate::search::engine::seq::OwnedIter::from_graph_and_search(__g, compiled)
                })
            })
    }};
    [$graph:expr, $($body:tt)*] => {{
        #[allow(unused_imports)]
        use $crate::search::dsl::*;
        let clusters = $crate::__search_clusters!(@acc [] $($body)*);
        $crate::search::query::compile(clusters).and_then(|compiled| {
            $crate::search::Session::from_search(compiled, $graph)
        })
    }};
    [$($body:tt)*] => {{
        #[allow(unused_imports)]
        use $crate::search::dsl::*;
        let clusters = $crate::__search_clusters!(@acc [] $($body)*);
        $crate::search::query::compile(clusters)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge;
    use crate::id;
    use crate::Id;

    type ER = edge::Undir<()>;

    #[test]
    fn morphism_ordering_iso_most_restrictive() {
        assert!(Morphism::Iso < Morphism::SubIso);
        assert!(Morphism::SubIso < Morphism::Mono);
        assert!(Morphism::Mono < Morphism::Homo);
    }

    #[test]
    fn search_macro_single_get_cluster() {
        let _s: Search<(), ER> = search![
            get(Morphism::Mono) {
                N(0) ^ N(1)
            }
        ].unwrap();
    }

    #[test]
    fn search_macro_multiple_clusters() {
        let _s: Search<(), ER> = search![
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            ban(Morphism::Mono) {
                n(0) ^ N(2),
                n(2) ^ n(1)
            }
        ].unwrap();
    }

    #[test]
    fn search_macro_with_type_annotation() {
        let _s = search![<(), ER>;
            get(Morphism::Iso) {
                N(0) ^ N(1),
                n(1) ^ N(2),
                n(0) ^ n(2)
            }
        ].unwrap();
    }

    #[test]
    fn search_macro_context_nodes() {
        let _s: Search<(), ER> = search![
            get(Morphism::Mono) {
                X(0) ^ N(1)
            },
            ban(Morphism::Mono) {
                x(0) ^ N(2),
                n(2) ^ n(1)
            }
        ].unwrap();
    }

    #[test]
    fn search_session_graph_form() {
        let g = crate::graph![<(), ER>; N(0) ^ N(1)].unwrap();
        let session = search![&g, get(Morphism::Mono) { N(0) ^ N(1) }].unwrap();
        let matches: Vec<_> = session.iter().collect();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn search_session_into_iter() {
        let g = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let session = search![&g, get(Morphism::Iso) {
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        }].unwrap();
        let mut count = 0;
        for _m in &session {
            count += 1;
        }
        assert_eq!(count, 6);
    }

    #[test]
    fn search_session_bound_pattern_errors() {
        let g = crate::graph![<(), ER>; N(0) ^ N(1)].unwrap();
        let result = search![&g, get(Morphism::Mono) { X(0) ^ N(1) }];
        assert!(matches!(result, Err(error::Search::BoundPatternInSession)));
    }

    #[test]
    fn search_session_count() {
        let g = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let session = search![&g, get(Morphism::Mono) { N(0) ^ N(1) }].unwrap();
        assert_eq!(session.iter().count(), 6);
    }

    #[test]
    fn search_inline_graph_for_loop() {
        let mut count = 0;
        for _m in search![graph![<(), ER>; N(0) ^ N(1), n(1) ^ N(2), n(0) ^ n(2)],
            get(Morphism::Iso) {
                N(0) ^ N(1),
                n(1) ^ N(2),
                n(0) ^ n(2)
            }
        ].unwrap() {
            count += 1;
        }
        assert_eq!(count, 6);
    }

    #[test]
    fn search_inline_graph_count() {
        let count = search![graph![<(), ER>; N(0) ^ N(1), n(1) ^ N(2), n(0) ^ n(2)],
            get(Morphism::Mono) { N(0) ^ N(1) }
        ].unwrap().count();
        assert_eq!(count, 6);
    }

    #[test]
    fn search_session_consuming_into_iter() {
        let g = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let mut count = 0;
        for _m in search![&g, get(Morphism::Iso) {
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        }].unwrap() {
            count += 1;
        }
        assert_eq!(count, 6);
    }

    #[test]
    fn search_session_consuming_count() {
        let g = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let count = search![&g, get(Morphism::Mono) { N(0) ^ N(1) }].unwrap().into_iter().count();
        assert_eq!(count, 6);
    }

    fn run_search(
        search: Search<(), ER>,
        target: crate::graph::Graph<(), ER>,
    ) -> usize {
        let Search::Resolved(r) = search
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let target = target.index(RevCsr);
        Seq::search(&query, &target).count()
    }

    #[test]
    fn iso_single_edge_matches_single_edge() {
        let target = crate::graph![<(), ER>; N(0) ^ N(1)].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Iso) { N(0) ^ N(1) }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 2);
    }

    #[test]
    fn iso_triangle_six_automorphisms() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Iso) {
                N(10) ^ N(11),
                n(11) ^ N(12),
                n(10) ^ n(12)
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 6);
    }

    #[test]
    fn iso_no_match_different_sizes() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(2) ^ N(3)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Iso) { N(0) ^ N(1) }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn iso_no_match_wrong_structure() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Iso) {
                N(0) ^ N(1),
                n(1) ^ N(2)
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn mono_edge_in_triangle() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) { N(0) ^ N(1) }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 6);
    }

    #[test]
    fn mono_path_in_triangle() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ N(1),
                n(1) ^ N(2)
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 6);
    }

    #[test]
    fn mono_no_match() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ N(1),
                n(1) ^ N(2),
                n(0) ^ n(2)
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn sub_iso_path_in_triangle_no_match() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::SubIso) {
                N(0) ^ N(1),
                n(1) ^ N(2)
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn sub_iso_vs_mono_difference() {
        let graph = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let target = graph.index(RevCsr);

        let mono_pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(10) ^ N(11),
                n(11) ^ N(12)
            }
        ];
        let Search::Resolved(r) = mono_pattern.unwrap()
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let mono_matches: Vec<_> = Seq::search(&query, &target).collect();
        assert_eq!(mono_matches.len(), 6);

        let sub_iso_pattern = search![<(), ER>;
            get(Morphism::SubIso) {
                N(10) ^ N(11),
                n(11) ^ N(12)
            }
        ];
        let Search::Resolved(r) = sub_iso_pattern.unwrap()
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let sub_iso_matches: Vec<_> = Seq::search(&query, &target).collect();
        assert_eq!(sub_iso_matches.len(), 0);
    }

    #[test]
    fn homo_allows_non_injective() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Homo) {
                N(0) ^ N(1)
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert!(count >= 2);
    }

    #[test]
    fn mixed_morphisms_across_clusters() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(2) ^ N(3)
        ].unwrap();

        let pattern = search![<(), ER>;
            get(Morphism::SubIso) {
                N(10) ^ N(12)
            },
            get(Morphism::Mono) {
                n(10) ^ N(11)
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 4);
    }

    type VER = edge::Undir<()>;

    fn run_valued_search(
        search: Search<i32, VER>,
        target: crate::graph::Graph<i32, VER>,
    ) -> usize {
        let Search::Resolved(r) = search
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let target = target.index(RevCsr);
        Seq::search(&query, &target).count()
    }

    #[test]
    fn neg_freestanding_val_bail() {
        let target = crate::graph![<i32, VER>;
            N(0).val(10) ^ N(1).val(20)
        ].unwrap();
        let pattern = search![<i32, VER>;
            get(Morphism::Mono) {
                !N_().val(10)
            }
        ];
        let count = run_valued_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn neg_freestanding_val_no_bail() {
        let pattern = search![<i32, VER>;
            get(Morphism::Mono) {
                !N_().val(99)
            }
        ];
        let Search::Resolved(r) = pattern.unwrap()
        else { panic!("unexpected context nodes") };
        let q = r.query();
        assert_eq!(q.search_order.len(), 0);
        assert_eq!(q.ban_clusters.len(), 1);
        assert_eq!(q.ban_clusters[0].ban_only_nodes.len(), 1);
    }

    #[test]
    fn neg_freestanding_no_pred_bail() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::SubIso) {
                !N_()
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn neg_connected_reject_third_neighbor() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ N(1) ^ !N_()
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn neg_connected_partial_reject() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(0) ^ N(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ N(1) ^ !N_()
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 2);
    }

    #[test]
    fn neg_connected_allow_when_no_third() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ N(1) ^ !N_()
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 2);
    }

    #[test]
    fn neg_connected_val_pred_reject() {
        let target = crate::graph![<i32, VER>;
            N(0).val(1) ^ N(1).val(42),
            n(0) ^ N(2).val(99)
        ].unwrap();
        let pattern = search![<i32, VER>;
            get(Morphism::Mono) {
                N(0) ^ !N(1).val(42)
            }
        ];
        let Search::Resolved(r) = pattern.unwrap()
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let target = target.index(RevCsr);
        let matches: Vec<_> = Seq::search(&query, &target).collect();
        for m in &matches {
            let mapped_0 = *m[0];
            assert_ne!(mapped_0, 0);
        }
    }

    #[test]
    fn neg_connected_val_pred_allow() {
        let target = crate::graph![<i32, VER>;
            N(0).val(1) ^ N(1).val(20)
        ].unwrap();
        let pattern = search![<i32, VER>;
            get(Morphism::Mono) {
                N(0) ^ !N(1).val(42)
            }
        ];
        let count = run_valued_search(pattern.unwrap(), target);
        assert_eq!(count, 2);
    }

    #[test]
    fn neg_context_val_freestanding() {
        let graph = crate::graph![<i32, VER>;
            N(0).val(42) ^ N(1).val(10)
        ].unwrap();
        let target = graph.index(RevCsr);
        let pattern = search![<i32, VER>;
            get(Morphism::Mono) {
                !X(0).val(42)
            }
        ];
        let Search::Unresolved(u) = pattern.unwrap()
        else { panic!("expected context nodes") };
        let ctx: &[(Id, Id)] = &[(0, 0)];
        let bindings = u.bind(ctx).unwrap().bindings;
        let query = u.query;
        let matches: Vec<_> = Seq::search_bound(&query, &target, bindings).collect();
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn neg_context_val_freestanding_no_match() {
        let pattern = search![<i32, VER>;
            get(Morphism::Mono) {
                !X(0).val(42)
            }
        ];
        let Search::Unresolved(u) = pattern.unwrap()
        else { panic!("expected context nodes") };
        assert_eq!(u.query().ban_clusters.len(), 1);
        assert_eq!(u.query().ban_clusters[0].ban_only_nodes.len(), 1);
    }

    #[test]
    fn positive_context_val_assertion() {
        let graph = crate::graph![<i32, VER>;
            N(0).val(42) ^ N(1).val(10)
        ].unwrap();
        let target = graph.index(RevCsr);
        let pattern = search![<i32, VER>;
            get(Morphism::Mono) {
                X(0).val(42) ^ N(1)
            }
        ];
        let Search::Unresolved(u) = pattern.unwrap()
        else { panic!("expected context nodes") };
        let ctx: &[(Id, Id)] = &[(0, 0)];
        let bindings = u.bind(ctx).unwrap().bindings;
        let query = u.query;
        let matches: Vec<_> = Seq::search_bound(&query, &target, bindings).collect();
        for m in &matches {
            let mapped = *m[0];
            let val = target.graph.nodes.get(id::N(mapped)).unwrap();
            assert_eq!(*val, 42);
        }
        assert!(!matches.is_empty());
    }

    #[test]
    fn contradictory_pos_neg_edge_same_pair_errors() {
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ N(1),
                n(0) & !E() ^ n(1)
            }
        ];
        assert!(matches!(
            pattern,
            Err(error::Search::Edge(error::Edge::Contradictory { .. }))
        ));
    }

    #[test]
    fn neg_connected_explicit_neg_edge_reject() {
        let target = crate::graph![<i32, VER>;
            N(0).val(1) ^ N(1).val(42)
        ].unwrap();
        let pattern = search![<i32, VER>;
            get(Morphism::Mono) {
                N(0) & !E() ^ !N_().val(42)
            }
        ];
        let Search::Resolved(r) = pattern.unwrap()
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let target = target.index(RevCsr);
        let matches: Vec<_> = Seq::search(&query, &target).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(*matches[0][0], 1);
    }

    #[test]
    fn neg_connected_explicit_neg_edge_reject_all() {
        let target = crate::graph![<i32, VER>;
            N(0).val(42) ^ N(1).val(42)
        ].unwrap();
        let pattern = search![<i32, VER>;
            get(Morphism::Mono) {
                N(0) & !E() ^ !N_().val(42)
            }
        ];
        let count = run_valued_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn neg_freestanding_isolated_subiso() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            N(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::SubIso) {
                N(0) ^ N(1)
            },
            ban(Morphism::SubIso) {
                !N_()
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn neg_two_negated_connected() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(0) ^ N(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ !N_(),
                n(0) ^ !N(1)
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 2);
    }

    #[test]
    fn neg_two_negated_connected_one_absent() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ !N_(),
                n(0) ^ !N(1)
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 2);
    }

    #[test]
    fn neg_context_with_edge_connected() {
        let graph = crate::graph![<i32, VER>;
            N(0).val(1) ^ N(1).val(2)
        ].unwrap();
        let target = graph.index(RevCsr);
        let pattern = search![<i32, VER>;
            get(Morphism::Mono) {
                N(0) ^ !X(1).val(2)
            }
        ];
        let Search::Unresolved(u) = pattern.unwrap()
        else { panic!("expected context nodes") };
        let ctx: &[(Id, Id)] = &[(1, 1)];
        let bindings = u.bind(ctx).unwrap().bindings;
        let query = u.query;
        let matches: Vec<_> = Seq::search_bound(&query, &target, bindings).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(*matches[0][0], 1);
    }

    #[test]
    fn homo_context_same_target_no_self_loop() {
        let graph = crate::graph![<(), ER>;
            N(0) ^ N(1)
        ].unwrap();
        let target = graph.index(RevCsr);
        let pattern = search![<(), ER>;
            get(Morphism::Homo) {
                X(0) % X(1)
            }
        ];
        let Search::Unresolved(u) = pattern.unwrap()
        else { panic!("expected context nodes") };
        let ctx: &[(Id, Id)] = &[(0, 1), (1, 1)];
        let bindings = u.bind(ctx).unwrap().bindings;
        let query = u.query;
        let matches: Vec<_> = Seq::search_bound(&query, &target, bindings).collect();
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn sub_iso_rejects_extra_edges_anydir() {
        type AER = crate::graph::edge::Anydir<()>;
        let graph = crate::graph![<(), AER>;
            N(0) >> (N(1) ^ N(2)),
            n(0) << n(1)
        ].unwrap();
        let target = graph.index(RevCsr);
        let pattern = search![<(), AER>; get(Morphism::SubIso) { X(0) % N(1) }];
        let Search::Unresolved(u) = pattern.unwrap()
        else { panic!("expected context nodes") };
        let ctx: &[(Id, Id)] = &[(0, 0)];
        let bindings = u.bind(ctx).unwrap().bindings;
        let query = u.query;
        let matches: Vec<_> = Seq::search_bound(&query, &target, bindings).collect();
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn neg_multi_cluster() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            ban(Morphism::Mono) {
                n(1) ^ !N_()
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn ban_cluster_positive_node_on_path() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(2) ^ N(3)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            ban(Morphism::Mono) {
                n(0) ^ N(2),
                n(2) ^ n(1)
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 6);
    }

    #[test]
    fn ban_cluster_rejects_when_triangle_exists() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            ban(Morphism::Mono) {
                n(0) ^ N(2),
                n(2) ^ n(1)
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn ban_cluster_partial_reject() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2),
            n(0) ^ N(3)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(10) ^ N(11)
            },
            ban(Morphism::Mono) {
                n(10) ^ N(12),
                n(12) ^ n(11)
            }
        ];
        let Search::Resolved(r) = pattern.unwrap()
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let target = target.index(RevCsr);
        let matches: Vec<_> = Seq::search(&query, &target).collect();
        for m in &matches {
            let n10 = *m[10];
            let n11 = *m[11];
            let pair = (std::cmp::min(n10, n11), std::cmp::max(n10, n11));
            assert!(pair == (0, 3) || pair == (2, 3));
        }
    }

    #[test]
    fn ban_cluster_neg_node_in_ban() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(2) ^ N(3)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ N(1)
            },
            ban(Morphism::Mono) {
                n(1) ^ !N_()
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn ban_cluster_no_ban_only_nodes() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ N(1),
                n(1) ^ N(2)
            },
            ban(Morphism::Mono) {
                n(0) ^ n(2)
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 2);
    }

    #[test]
    fn ban_cluster_shared_edge_still_enforces() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) ^ N(1),
                n(1) ^ N(2)
            },
            ban(Morphism::Mono) {
                n(0) ^ n(2)
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn neg_homo_allows_duplicate_mapping() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Homo) {
                N(0) ^ N(1) ^ !N_()
            }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    type DER = edge::Dir<()>;

    fn run_dir_search(
        search: Search<(), DER>,
        target: crate::graph::Graph<(), DER>,
    ) -> usize {
        let Search::Resolved(r) = search
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let target = target.index(RevCsr);
        Seq::search(&query, &target).count()
    }

    fn dir_graph(nodes: &[u32], edges: &[(u32, u32)]) -> crate::graph::Dir0 {
        use crate::graph::edge::dir;
        let ns: Vec<u32> = nodes.to_vec();
        let es: Vec<dir::E<u32>> = edges.iter().map(|&(a, b)| dir::E::D(a, b)).collect();
        crate::graph::Dir0::try_from((ns, es)).unwrap()
    }

    #[test]
    fn dir_basic_match() {
        let target = dir_graph(&[0, 1], &[(0, 1)]);
        let pattern = search![<(), DER>;
            get(Morphism::Mono) { N(0) >> N(1) }
        ];
        let count = run_dir_search(pattern.unwrap(), target);
        assert_eq!(count, 1);
    }

    #[test]
    fn dir_reverse_uses_shl() {
        let target = dir_graph(&[0, 1], &[(0, 1)]);
        let pattern = search![<(), DER>;
            get(Morphism::Mono) { N(0) << N(1) }
        ];
        let Search::Resolved(r) = pattern.unwrap()
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let target = target.index(RevCsr);
        let matches: Vec<_> = Seq::search(&query, &target).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(*matches[0][0], 1);
        assert_eq!(*matches[0][1], 0);
    }

    #[test]
    fn dir_iso_one_automorphism() {
        let target = dir_graph(&[0, 1], &[(0, 1)]);
        let pattern = search![<(), DER>;
            get(Morphism::Iso) { N(0) >> N(1) }
        ];
        let count = run_dir_search(pattern.unwrap(), target);
        assert_eq!(count, 1);
    }

    #[test]
    fn dir_iso_bidirectional_two_autos() {
        let target = dir_graph(&[0, 1], &[(0, 1), (1, 0)]);
        let pattern = search![<(), DER>;
            get(Morphism::Iso) { N(0) >> N(1), n(1) >> n(0) }
        ];
        let count = run_dir_search(pattern.unwrap(), target);
        assert_eq!(count, 2);
    }

    #[test]
    fn dir_triangle_mono() {
        let target = dir_graph(&[0, 1, 2], &[(0, 1), (1, 2), (2, 0)]);
        let pattern = search![<(), DER>;
            get(Morphism::Mono) {
                N(0) >> N(1),
                n(1) >> N(2)
            }
        ];
        let count = run_dir_search(pattern.unwrap(), target);
        assert_eq!(count, 3);
    }

    #[test]
    fn dir_contradictory_pos_neg_edge_errors() {
        let pattern = search![<(), DER>;
            get(Morphism::Mono) {
                N(0) >> N(1),
                n(0) & !E() >> n(1)
            }
        ];
        assert!(matches!(
            pattern,
            Err(error::Search::Edge(error::Edge::Contradictory { .. }))
        ));
    }

    type AER = edge::Anydir<()>;

    fn run_anydir_search(
        search: Search<(), AER>,
        target: crate::graph::Graph<(), AER>,
    ) -> usize {
        let Search::Resolved(r) = search
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let target = target.index(RevCsr);
        Seq::search(&query, &target).count()
    }

    fn anydir_graph(
        nodes: &[u32],
        dir_edges: &[(u32, u32)],
        undir_edges: &[(u32, u32)],
    ) -> crate::graph::Anydir0 {
        use crate::graph::edge::anydir;
        let ns: Vec<u32> = nodes.to_vec();
        let mut es: Vec<anydir::E<u32>> = Vec::new();
        for &(a, b) in dir_edges {
            es.push(anydir::E::D(a, b));
        }
        for &(a, b) in undir_edges {
            es.push(anydir::E::U(a, b));
        }
        crate::graph::Anydir0::try_from((ns, es)).unwrap()
    }

    #[test]
    fn anydir_mixed_match() {
        let target = anydir_graph(&[0, 1, 2], &[(0, 1)], &[(1, 2)]);
        let pattern = search![<(), AER>;
            get(Morphism::Mono) {
                N(0) >> N(1),
                n(1) ^ N(2)
            }
        ];
        let count = run_anydir_search(pattern.unwrap(), target);
        assert_eq!(count, 1);
    }

    #[test]
    fn anydir_dir_only() {
        let target = anydir_graph(&[0, 1], &[(0, 1)], &[]);
        let pattern = search![<(), AER>;
            get(Morphism::Mono) { N(0) >> N(1) }
        ];
        let count = run_anydir_search(pattern.unwrap(), target);
        assert_eq!(count, 1);
    }

    #[test]
    fn anydir_undir_only() {
        let target = anydir_graph(&[0, 1], &[], &[(0, 1)]);
        let pattern = search![<(), AER>;
            get(Morphism::Mono) { N(0) ^ N(1) }
        ];
        let count = run_anydir_search(pattern.unwrap(), target);
        assert_eq!(count, 2);
    }

    #[test]
    fn ban_duplicates_get_edge_undir_errors() {
        let pattern = search![<(), ER>;
            get(Morphism::Mono) { N(0) ^ N(1) },
            ban(Morphism::Mono) { n(0) ^ n(1) }
        ];
        assert!(matches!(
            pattern,
            Err(error::Search::Edge(error::Edge::RedundantInBan { src: 0, tgt: 1 }))
        ));
    }

    #[test]
    fn ban_duplicates_get_edge_dir_same_slot_errors() {
        let pattern = search![<(), DER>;
            get(Morphism::Mono) { N(0) >> N(1) },
            ban(Morphism::Mono) { n(0) >> n(1) }
        ];
        assert!(matches!(
            pattern,
            Err(error::Search::Edge(error::Edge::RedundantInBan { src: 0, tgt: 1 }))
        ));
    }

    #[test]
    fn ban_different_slot_dir_not_contradictory() {
        let target = dir_graph(&[0, 1], &[(0, 1)]);
        let pattern = search![<(), DER>;
            get(Morphism::Mono) { N(0) >> N(1) },
            ban(Morphism::Mono) { n(0) << n(1) }
        ];
        let count = run_dir_search(pattern.unwrap(), target);
        assert_eq!(count, 1);
    }

    #[test]
    fn ban_mono_with_ban_only_not_subsumed() {
        let pattern = search![<(), DER>;
            get(Morphism::Mono) { N(0) >> N(1) >> N(2) },
            ban(Morphism::Mono) { n(0) >> N_() }
        ];
        assert!(pattern.is_ok());
    }

    #[test]
    fn ban_homo_with_ban_only_subsumed() {
        let pattern = search![<(), DER>;
            get(Morphism::Mono) { N(0) >> N(1) >> N(2) },
            ban(Morphism::Homo) { n(0) >> N_() }
        ];
        assert!(matches!(
            pattern,
            Err(error::Search::Cluster(error::Cluster::Subsumed))
        ));
    }

    #[test]
    fn ban_shared_node_only_subsumed() {
        let pattern = search![<(), DER>;
            get(Morphism::Mono) { N(0) >> N(1) },
            ban(Morphism::Mono) { n(0) }
        ];
        assert!(matches!(
            pattern,
            Err(error::Search::Cluster(error::Cluster::Subsumed))
        ));
    }

    #[test]
    fn ban_not_subsumed_different_slot_with_ban_only_node() {
        let target = dir_graph(&[0, 1, 2], &[(0, 1), (1, 2)]);
        let pattern = search![<(), DER>;
            get(Morphism::Mono) { N(0) >> N(1), n(1) >> N(2) },
            ban(Morphism::Mono) { n(1) << N_() }
        ];
        let count = run_dir_search(pattern.unwrap(), target);
        assert_eq!(count, 1);
    }

    #[test]
    fn ban_anyslot_undir_covered_by_get() {
        let pattern = search![<(), ER>;
            get(Morphism::Mono) { N(0) ^ N(1) },
            ban(Morphism::Mono) { n(0) % n(1) }
        ];
        assert!(matches!(
            pattern,
            Err(error::Search::Edge(error::Edge::CoveredByGet { src: 0, tgt: 1 }))
        ));
    }

    #[test]
    fn ban_anyslot_dir_one_slot_valid() {
        let target = dir_graph(&[0, 1], &[(0, 1)]);
        let pattern = search![<(), DER>;
            get(Morphism::Mono) { N(0) >> N(1) },
            ban(Morphism::Mono) { n(0) % n(1) }
        ];
        let count = run_dir_search(pattern.unwrap(), target);
        assert_eq!(count, 1);
    }

    #[test]
    fn ban_anyslot_dir_all_slots_covered() {
        let pattern = search![<(), DER>;
            get(Morphism::Mono) { N(0) >> N(1) },
            get(Morphism::Mono) { n(0) << n(1) },
            ban(Morphism::Mono) { n(0) % n(1) }
        ];
        assert!(matches!(
            pattern,
            Err(error::Search::Edge(error::Edge::CoveredByGet { src: 0, tgt: 1 }))
        ));
    }

    #[test]
    fn ban_anyslot_anydir_one_slot_valid() {
        let target = anydir_graph(&[0, 1], &[(0, 1)], &[]);
        let pattern = search![<(), AER>;
            get(Morphism::Mono) { N(0) >> N(1) },
            ban(Morphism::Mono) { n(0) % n(1) }
        ];
        let count = run_anydir_search(pattern.unwrap(), target);
        assert_eq!(count, 1);
    }

    #[test]
    fn ban_specific_slot_sub_iso_covered() {
        let pattern = search![<(), DER>;
            get(Morphism::SubIso) { N(0) >> N(1) },
            ban(Morphism::Mono) { n(0) << n(1) }
        ];
        assert!(matches!(
            pattern,
            Err(error::Search::Edge(error::Edge::CoveredByGet { src: 0, tgt: 1 }))
        ));
    }

    #[test]
    fn ban_specific_slot_mono_different_slot_valid() {
        let target = dir_graph(&[0, 1], &[(0, 1)]);
        let pattern = search![<(), DER>;
            get(Morphism::Mono) { N(0) >> N(1) },
            ban(Morphism::Mono) { n(0) << n(1) }
        ];
        let count = run_dir_search(pattern.unwrap(), target);
        assert_eq!(count, 1);
    }

    #[test]
    fn ban_sub_iso_anyslot_survives_multi_edge() {
        let target = anydir_graph(&[0, 1, 2, 3], &[(0, 1), (1, 0)], &[]);
        let pattern = search![<(), AER>;
            get(Morphism::Mono) { N(0), N(1) },
            ban(Morphism::SubIso) { n(0) % n(1) }
        ];
        let count = run_anydir_search(pattern.unwrap(), target);
        assert_eq!(count, 12);
    }

    #[test]
    fn ban_sub_iso_anyslot_rejects_single_edge() {
        let target = anydir_graph(&[0, 1, 2], &[(0, 1)], &[]);
        let pattern = search![<(), AER>;
            get(Morphism::Mono) { N(0), N(1) },
            ban(Morphism::SubIso) { n(0) % n(1) }
        ];
        let count = run_anydir_search(pattern.unwrap(), target);
        assert_eq!(count, 4);
    }

    #[test]
    fn ban_sub_iso_extra_edge_unspecified_pair() {
        let target = anydir_graph(&[0, 1, 2], &[(0, 1), (1, 0), (0, 2)], &[]);
        let pattern = search![<(), AER>;
            get(Morphism::Mono) { N(0) >> N(1), N(2) },
            ban(Morphism::SubIso) { n(0) % n(1), n(2) }
        ];
        let count = run_anydir_search(pattern.unwrap(), target);
        assert_eq!(count, 3);
    }

    #[test]
    fn ban_sub_iso_no_extra_edge_unspecified_pair() {
        let target = anydir_graph(&[0, 1, 2], &[(0, 1), (1, 0)], &[]);
        let pattern = search![<(), AER>;
            get(Morphism::Mono) { N(0) >> N(1), N(2) },
            ban(Morphism::SubIso) { n(0) % n(1), n(2) }
        ];
        let count = run_anydir_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    // =========================================================================
    // Any-edge operator (%) tests
    // =========================================================================

    #[test]
    fn any_edge_undir() {
        let target = crate::graph![<(), ER>; N(0) ^ N(1)].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) { N(0) % N(1) }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 2);
    }

    #[test]
    fn any_edge_dir() {
        let target = dir_graph(&[0, 1], &[(0, 1)]);
        let pattern = search![<(), DER>;
            get(Morphism::Mono) { N(0) % N(1) }
        ];
        let count = run_dir_search(pattern.unwrap(), target);
        assert_eq!(count, 2);
    }

    #[test]
    fn any_edge_anydir() {
        let target = anydir_graph(&[0, 1, 2], &[(0, 1)], &[(1, 2)]);
        let pattern = search![<(), AER>;
            get(Morphism::Mono) { N(0) % N(1) }
        ];
        let count = run_anydir_search(pattern.unwrap(), target);
        assert_eq!(count, 4);
    }

    #[test]
    fn any_edge_negated() {
        let graph = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(0) ^ N(2)
        ].unwrap();
        let target = graph.index(RevCsr);
        let pattern = search![<(), ER>;
            get(Morphism::Mono) {
                N(0) % N(1),
                n(0) & !E() % N(2)
            }
        ];
        let Search::Resolved(r) = pattern.unwrap()
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let matches: Vec<_> = Seq::search(&query, &target).collect();
        for m in &matches {
            let n0 = *m[0];
            let n2 = *m[2];
            assert!(!target.graph.is_adjacent(n0, n2));
        }
    }

    #[test]
    fn any_edge_with_pred() {
        type VER2 = edge::Undir<i32>;
        let graph = crate::graph![<(), VER2>;
            N(0) ^ N(1),
            n(0) & E().val(5) ^ N(2)
        ].unwrap();
        let target = graph.index(RevCsr);
        let pattern = search![<(), VER2>;
            get(Morphism::Mono) {
                N(0) & E().test(|v| *v > 0) % N(1)
            }
        ];
        let Search::Resolved(r) = pattern.unwrap()
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let matches: Vec<_> = Seq::search(&query, &target).collect();
        assert!(!matches.is_empty());
    }

    #[test]
    fn any_edge_in_ban() {
        let target = crate::graph![<(), ER>;
            N(0) ^ N(1),
            n(1) ^ N(2),
            n(0) ^ n(2)
        ].unwrap();
        let pattern = search![<(), ER>;
            get(Morphism::Mono) { N(0) % N(1) },
            ban(Morphism::Mono) { n(0) % N(2) }
        ];
        let count = run_search(pattern.unwrap(), target);
        assert_eq!(count, 0);
    }

    #[test]
    fn any_edge_filter_dir() {
        let target = dir_graph(&[0, 1], &[(0, 1)]);
        let pattern = search![<(), DER>;
            get(Morphism::Mono) {
                N(0) % N(1),
                n(0) & !E() << n(1)
            }
        ];
        let Search::Resolved(r) = pattern.unwrap()
        else { panic!("unexpected context nodes") };
        let query = r.query;
        let target = target.index(RevCsr);
        let matches: Vec<_> = Seq::search(&query, &target).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(*matches[0][0], 0);
        assert_eq!(*matches[0][1], 1);
    }
}
