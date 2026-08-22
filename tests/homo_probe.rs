use grw::search::{Search, Seq, RevCsr};
use grw::graph::dsl::LocalId;

type ER = grw::edge::Undir<()>;

// PROBE 1: mixed injectivity — is the match set symmetric under the
// automorphism swapping target leaves a<->b and pattern homo nodes 0<->1?
// Any consistent semantics must produce a symmetric set.
#[test]
fn probe_mixed_homo_mono_symmetry() {
    // target: star — c(0) joined to a(1), b(2); plus d(3) on c so the
    // mono leaf has somewhere to go: edges c-a, c-b, c-d
    let g = grw::graph![<(), ER>;
        N(0) ^ N(1),
        n(0) ^ N(2),
        n(0) ^ N(3)
    ].unwrap();
    let t = g.index(RevCsr);

    // pattern: 2(mono hub) — 3(mono leaf), plus homo 0,1 hanging off hub
    let p = grw::search![<(), ER>;
        get(Morphism::Mono) { N(2) ^ N(3) },
        get(Morphism::Homo) { N(0) ^ n(2), N(1) ^ n(2) }
    ];
    let Search::Resolved(r) = p.unwrap() else { panic!() };
    let ms: Vec<[u32; 4]> = Seq::search(&r.query(), &t)
        .map(|m| [ *m[0] as u32, *m[1] as u32, *m[2] as u32, *m[3] as u32 ])
        .collect();

    println!("PROBE1 matches ({}):", ms.len());
    for m in &ms { println!("  0->{} 1->{} 2->{} 3->{}", m[0], m[1], m[2], m[3]); }

    // symmetry check: swapping pattern nodes 0,1 must preserve the set
    let mut set: std::collections::BTreeSet<[u32;4]> = ms.iter().cloned().collect();
    let swapped: std::collections::BTreeSet<[u32;4]> =
        ms.iter().map(|m| [m[1], m[0], m[2], m[3]]).collect();
    assert_eq!(set, swapped, "match set not symmetric under homo-node swap 0<->1");

    // and mono-leaf freshness must be decided uniformly:
    // either NO match has m3 == m0/m1, or the full homo product appears.
    let sharing = ms.iter().filter(|m| m[3]==m[0] || m[3]==m[1]).count();
    println!("PROBE1 sharing-with-mono-leaf: {} of {}", sharing, ms.len());
    assert_eq!(set.len(), 30, "hub-anchored 27 + 3 leaf-anchored (m2 on a leaf, everything else on the hub)");
}

// PROBE 2: Mono cluster, two path edges forced through the SAME interior.
// Cross-path interior injectivity: enforced at match time, opt-in, or absent?
#[test]
fn probe_mono_two_paths_shared_interior() {
    // target: a(0)-x(1), x-b(2), x-c(3): both a..b and b..c must route via x
    let g = grw::graph![<(), ER>;
        N(0) ^ N(1),
        n(1) ^ N(2),
        n(1) ^ N(3)
    ].unwrap();
    let t = g.index(RevCsr);

    let p = grw::search![<(), ER>;
        get(Morphism::Mono) {
            N(0) ^ ..N(2).dfs().len(2..3),
            n(2) ^ ..N(3).dfs().len(2..3)
        }
    ];
    let Search::Resolved(r) = p.unwrap() else { panic!() };
    let ms: Vec<_> = Seq::search(&r.query(), &t).collect();
    println!("PROBE2 matches: {}", ms.len());
    for m in &ms {
        println!("  0->{:?} 2->{:?} 3->{:?} path0={:?} path1={:?}",
            m.get(LocalId(0)), m.get(LocalId(2)), m.get(LocalId(3)),
            m.path(0), m.path(1));
    }
    for mut m in Seq::search(&r.query(), &t) {
        let mut inj = 0usize;
        loop {
            if m.paths_are_injective() { inj += 1; }
            if !m.next_paths() { break; }
        }
        println!("PROBE2 injective combos for this match: {inj}");
    }
}

// PROBE 3: homo merge of ADJACENT pattern nodes needs a target self-loop.
// Graph model self-loop policy + engine behavior.
#[test]
fn probe_homo_adjacent_merge_selfloop() {
    let selfloop = grw::graph![<(), ER>; N(0) ^ n(0)];
    println!("PROBE3 self-loop build: {:?}", selfloop.as_ref().map(|g| (g.node_count(), g.edge_count())).map_err(|e| format!("{e}")));

    let g = grw::graph![<(), ER>; N(0) ^ N(1)].unwrap();
    let t = g.index(RevCsr);
    let p = grw::search![<(), ER>;
        get(Morphism::Homo) { N(0) ^ N(1) }
    ];
    let Search::Resolved(r) = p.unwrap() else { panic!() };
    let merged = Seq::search(&r.query(), &t)
        .filter(|m| m.get(LocalId(0)) == m.get(LocalId(1)))
        .count();
    println!("PROBE3 merged-adjacent matches on loop-free target: {merged}");
    assert_eq!(merged, 0, "adjacent homo nodes merged without a self-loop in target");
}

// PROBE 1b: same logical shape as probe 1, but homo nodes carry pendants so
// the degree-greedy order binds them BEFORE the mono leaf. If injectivity
// were declaration-order-independent, the match set must stay symmetric
// under swapping homo nodes 0<->1.
#[test]
fn probe_mixed_homo_mono_order_flipped() {
    let g = grw::graph![<(), ER>;
        N(0) ^ N(1),
        n(0) ^ N(2)
    ].unwrap();
    let t = g.index(RevCsr);

    let p = grw::search![<(), ER>;
        get(Morphism::Mono) { N(2) ^ N(3) },
        get(Morphism::Homo) {
            N(0) ^ n(2),
            N(1) ^ n(2),
            N(4) ^ n(0),
            N(5) ^ n(1)
        }
    ];
    let Search::Resolved(r) = p.unwrap() else { panic!() };
    let ms: Vec<[u32; 4]> = Seq::search(&r.query(), &t)
        .map(|m| [ *m[0] as u32, *m[1] as u32, *m[2] as u32, *m[3] as u32 ])
        .collect();
    println!("PROBE1b matches ({}):", ms.len());
    for m in &ms { println!("  0->{} 1->{} 2->{} 3->{}", m[0], m[1], m[2], m[3]); }
    let set: std::collections::BTreeSet<[u32;4]> = ms.iter().cloned().collect();
    let swapped: std::collections::BTreeSet<[u32;4]> =
        ms.iter().map(|m| [m[1], m[0], m[2], m[3]]).collect();
    assert_eq!(set, swapped, "match set changed under homo-node swap: injectivity is order-dependent");
}

// PROBE 2b: same two-paths-shared-interior shape as probe 2, but Homo.
// If interior disjointness is morphism-driven, Homo must find matches.
#[test]
fn probe_homo_two_paths_shared_interior() {
    let g = grw::graph![<(), ER>;
        N(0) ^ N(1),
        n(1) ^ N(2),
        n(1) ^ N(3)
    ].unwrap();
    let t = g.index(RevCsr);
    let p = grw::search![<(), ER>;
        get(Morphism::Homo) {
            N(0) ^ ..N(2).dfs().len(2..3),
            n(2) ^ ..N(3).dfs().len(2..3)
        }
    ];
    let Search::Resolved(r) = p.unwrap() else { panic!() };
    let n = Seq::search(&r.query(), &t).count();
    println!("PROBE2b homo shared-interior matches: {n}");
}

// PROBE 3b: target HAS a self-loop — homo merge of adjacent nodes should
// now be possible (edge preservation satisfied by the loop).
#[test]
fn probe_homo_adjacent_merge_with_selfloop() {
    let g = grw::graph![<(), ER>;
        N(0) ^ n(0),
        n(0) ^ N(1)
    ].unwrap();
    let t = g.index(RevCsr);
    let p = grw::search![<(), ER>;
        get(Morphism::Homo) { N(0) ^ N(1) }
    ];
    let Search::Resolved(r) = p.unwrap() else { panic!() };
    let merged = Seq::search(&r.query(), &t)
        .filter(|m| m.get(LocalId(0)) == m.get(LocalId(1)))
        .count();
    println!("PROBE3b merged-adjacent with self-loop present: {merged}");
}

// PROBE 4: Epi (surjective) with a path edge — do path interiors count as
// "covering" target nodes?
#[test]
fn probe_epi_path_interior_coverage() {
    // target: a(0)-x(1)-b(2): explicit endpoints cover a,b; interior x
    // only via the path.
    let g = grw::graph![<(), ER>;
        N(0) ^ N(1),
        n(1) ^ N(2)
    ].unwrap();
    let t = g.index(RevCsr);
    let p = grw::search![<(), ER>;
        get(Morphism::Epi) { N(0) ^ ..N(1).dfs().len(2..3) }
    ];
    let Search::Resolved(r) = p.unwrap() else { panic!() };
    let n = Seq::search(&r.query(), &t).count();
    println!("PROBE4 epi-with-path matches (interior covers x?): {n}");
}

// PROBE 5: pigeonhole precompute counts homo nodes — minimal repro.
// 3 pattern nodes (1 mono + 2 homo), 2-node target: m0=a, m1=m2=b is a
// legal match, but any_injective makes precompute require target >= 3.
#[test]
fn probe_pigeonhole_counts_homo_nodes() {
    let g = grw::graph![<(), ER>; N(0) ^ N(1)].unwrap();
    let t = g.index(RevCsr);
    let p = grw::search![<(), ER>;
        get(Morphism::Mono) { N(0) },
        get(Morphism::Homo) { N(1) ^ N(2) }
    ];
    let Search::Resolved(r) = p.unwrap() else { panic!() };
    let n = Seq::search(&r.query(), &t).count();
    println!("PROBE5 matches: {n}");
    assert_eq!(n, 4, "1 mono free node (2) x homo adjacent pair without self-loop (2)");
}

// PROBE 1c: order-flipped mixed query, target padded past the pigeonhole.
#[test]
fn probe_mixed_order_flipped_padded() {
    let g = grw::graph![<(), ER>;
        N(0) ^ N(1),
        n(0) ^ N(2),
        N(3), N(4), N(5)
    ].unwrap();
    let t = g.index(RevCsr);
    let p = grw::search![<(), ER>;
        get(Morphism::Mono) { N(2) ^ N(3) },
        get(Morphism::Homo) {
            N(0) ^ n(2),
            N(1) ^ n(2),
            N(4) ^ n(0),
            N(5) ^ n(1)
        }
    ];
    let Search::Resolved(r) = p.unwrap() else { panic!() };
    let ms: Vec<[u32; 4]> = Seq::search(&r.query(), &t)
        .map(|m| [ *m[0] as u32, *m[1] as u32, *m[2] as u32, *m[3] as u32 ])
        .collect();
    println!("PROBE1c matches ({}):", ms.len());
    for m in &ms { println!("  0->{} 1->{} 2->{} 3->{}", m[0], m[1], m[2], m[3]); }
    let set: std::collections::BTreeSet<[u32;4]> = ms.iter().cloned().collect();
    let swapped: std::collections::BTreeSet<[u32;4]> =
        ms.iter().map(|m| [m[1], m[0], m[2], m[3]]).collect();
    assert_eq!(set, swapped, "match set changed under homo-node swap: injectivity is bind-order-dependent");
}
