use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // target: triangle + pendant.
    // Iso { N(10)^N(11) }: nodes with degree exactly 1? None in triangle.
    //   All triangle nodes have degree 2. Pendant nodes: 3 has degree 1, 4 has degree 1.
    //   Wait — Iso means the ENTIRE graph must match the ENTIRE pattern.
    //   But it's mixed with Mono...
    //   Actually: Iso degree check is per-node. Node 10 needs target node with
    //   degree == pattern_degree(10) = 1. In the target, nodes 3 and 4 have degree 1.
    //   Node 11 also needs degree 1. So N(10)→3, N(11)→4 or vice versa.
    //   Mono { N(12)^N(13) }: target nodes with degree ≥ 1, not already used.
    //   Remaining: 0,1,2. All have degree 2 ≥ 1. Pick any 2 with an edge.
    //
    // Q: This is weird. The "Iso" morphism on an isolated subpattern doesn't mean
    //    full-graph Iso — it means those specific nodes enforce Iso-level constraints
    //    (exact degree, SubIso coverage). The result is NOT a true graph isomorphism.

    println!("--- what Iso means on sub-pattern ---");

    println!("  compare: single Iso cluster with all 4 nodes");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2),
        N(3) ^ N(4)
    ]?;

    trace!(verbose &g, search![
        get(Iso) {
            N(10) ^ N(11),
            N(12) ^ N(13)
        }
    ]);

    Ok(())
}
