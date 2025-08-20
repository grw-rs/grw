use grw::*;
use grw::search::{self, Session};

type UER = grw::graph::edge::Undir<()>;

fn count<NV: Clone, ER: graph::Edge + 'static>(
    g: &graph::Graph<NV, ER>,
    compiled: search::Search<NV, ER>,
) -> usize
where
    ER::Val: Clone,
{
    let session = Session::from_search(compiled, g).unwrap();
    session.iter().count()
}

// ============================================================================
// § Epi basics — surjective, not injective
// ============================================================================

#[test]
fn epi_single_edge_exact_cover() {
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let result = count(&g, search![<(), UER>;
        get(Epi) { N(0) ^ N(1) }
    ].unwrap());

    println!("epi_single_edge_exact_cover: {result}");
    assert_eq!(result, 2, "2 nodes, 2 pattern nodes → surjective possible");
}

#[test]
fn epi_fails_not_enough_pattern_nodes() {
    let g: graph::Undir0 = graph![N(0) ^ N(1), N(2)].unwrap();

    let result = count(&g, search![<(), UER>;
        get(Epi) { N(0) ^ N(1) }
    ].unwrap());

    println!("epi_fails_not_enough: {result}");
    assert_eq!(result, 0, "3 target nodes, 2 pattern nodes → can't cover");
}

#[test]
fn epi_triangle_pattern_on_triangle() {
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        n(1) ^ N(2),
        n(0) ^ n(2)
    ].unwrap();

    let result = count(&g, search![<(), UER>;
        get(Epi) { N(0) ^ N(1), n(1) ^ N(2), n(0) ^ n(2) }
    ].unwrap());

    println!("epi_triangle_on_triangle: {result}");
    assert!(result > 0, "triangle on triangle → surjective");
}

#[test]
fn epi_allows_non_injective() {
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let result = count(&g, search![<(), UER>;
        get(Epi) { N(0) ^ N(1), n(0) ^ N(2) }
    ].unwrap());

    println!("epi_allows_non_injective: {result}");
    assert!(result > 0, "3 pattern nodes on 2 target nodes → Epi allows collapse");
}

#[test]
fn epi_rejects_uncovered_node() {
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        n(1) ^ N(2),
        n(0) ^ n(2),
        N(3)
    ].unwrap();

    let result = count(&g, search![<(), UER>;
        get(Epi) { N(0) ^ N(1), n(1) ^ N(2), n(0) ^ n(2) }
    ].unwrap());

    println!("epi_rejects_uncovered: {result}");
    assert_eq!(result, 0, "node 3 never covered → surjective fails");
}

// ============================================================================
// § Epi vs Homo — same pattern, different morphism
// ============================================================================

#[test]
fn epi_vs_homo_extra_target_node() {
    let g: graph::Undir0 = graph![N(0) ^ N(1), N(2)].unwrap();

    let epi = count(&g, search![<(), UER>;
        get(Epi) { N(0) ^ N(1) }
    ].unwrap());

    let homo = count(&g, search![<(), UER>;
        get(Homo) { N(0) ^ N(1) }
    ].unwrap());

    println!("epi={epi} homo={homo}");
    assert_eq!(epi, 0, "Epi fails: can't cover node 2");
    assert!(homo > 0, "Homo succeeds: no surjectivity requirement");
}

// ============================================================================
// § EpiMono — injective + surjective (bijective, non-induced)
// ============================================================================

#[test]
fn epimono_exact_bijection() {
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let result = count(&g, search![<(), UER>;
        get(EpiMono) { N(0) ^ N(1) }
    ].unwrap());

    println!("epimono_exact_bijection: {result}");
    assert_eq!(result, 2, "bijection on exact-size graph");
}

#[test]
fn epimono_fails_too_few_target() {
    let g: graph::Undir0 = graph![N(0) ^ N(1)].unwrap();

    let result = count(&g, search![<(), UER>;
        get(EpiMono) { N(0) ^ N(1), n(0) ^ N(2) }
    ].unwrap());

    println!("epimono_fails_too_few_target: {result}");
    assert_eq!(result, 0, "3 pattern nodes, 2 target → injective impossible");
}

#[test]
fn epimono_fails_too_many_target() {
    let g: graph::Undir0 = graph![N(0) ^ N(1), N(2)].unwrap();

    let result = count(&g, search![<(), UER>;
        get(EpiMono) { N(0) ^ N(1) }
    ].unwrap());

    println!("epimono_fails_too_many_target: {result}");
    assert_eq!(result, 0, "2 pattern, 3 target → surjective impossible");
}

#[test]
fn epimono_allows_extra_edges() {
    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        n(1) ^ N(2),
        n(0) ^ n(2)
    ].unwrap();

    let epimono = count(&g, search![<(), UER>;
        get(EpiMono) { N(0) ^ N(1), n(1) ^ N(2) }
    ].unwrap());

    let iso = count(&g, search![<(), UER>;
        get(Iso) { N(0) ^ N(1), n(1) ^ N(2) }
    ].unwrap());

    println!("epimono={epimono} iso={iso}");
    assert!(epimono > 0, "EpiMono: bijective, extra edge 0-2 allowed");
    assert_eq!(iso, 0, "Iso: induced, extra edge 0-2 rejects");
}

// ============================================================================
// § Meet lattice — Mono+Epi node convergence
// ============================================================================

#[test]
fn meet_mono_epi_produces_epimono() {
    assert_eq!(Mono.meet(Epi), EpiMono);
    assert_eq!(Epi.meet(Mono), EpiMono);
}

#[test]
fn meet_subiso_epi_produces_iso() {
    assert_eq!(SubIso.meet(Epi), Iso);
    assert_eq!(Epi.meet(SubIso), Iso);
}

#[test]
fn meet_idempotent() {
    assert_eq!(Epi.meet(Epi), Epi);
    assert_eq!(EpiMono.meet(EpiMono), EpiMono);
    assert_eq!(Mono.meet(Mono), Mono);
    assert_eq!(Iso.meet(Iso), Iso);
}

#[test]
fn meet_homo_identity() {
    assert_eq!(Homo.meet(Epi), Epi);
    assert_eq!(Homo.meet(Mono), Mono);
    assert_eq!(Homo.meet(Iso), Iso);
    assert_eq!(Homo.meet(EpiMono), EpiMono);
    assert_eq!(Homo.meet(SubIso), SubIso);
}
