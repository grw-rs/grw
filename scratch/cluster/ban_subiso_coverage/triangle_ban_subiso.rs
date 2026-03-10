use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Does ban morphism affect SubIso coverage checks?
    //
    // ban_node_feasible does NOT have SubIso coverage logic.
    // It only checks edge existence (positive/negated).
    // So ban(SubIso) and ban(Mono) differ ONLY in injectivity
    // of ban_only nodes, NOT in coverage.
    //
    // Is this correct? Should ban(SubIso) also enforce that
    // the ban_only node has no EXTRA edges to shared nodes
    // beyond what the ban pattern specifies?

    // target: triangle 0-1-2
    // get(Mono): edge 0-1
    // ban(SubIso): n(1) ^ N(3) — "ban if matched[1] has extra neighbor"
    //
    // For match {0→0,1→1}: ban needs N(3) adj to 1, N(3)∉{0,1}. → N(3)→2 works.
    //   Under SubIso, should we also check that 2 has no EXTRA edges back to matched nodes?
    //   Node 2 is adj to 0 (mapped to n(0)), but ban doesn't mention n(0)-N(3) edge.
    //   Does this "uncovered" edge disqualify the ban mapping?
    //
    // Current: ban_node_feasible only checks edges listed in ban.edge_indices.
    //   It does NOT do SubIso coverage. So the extra 2-0 edge is ignored.

    println!("--- ban(SubIso) vs ban(Mono) on triangle ---");

    println!("  ban(SubIso):");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(SubIso) {
            n(1) ^ N(3)
        }
    ]);

    Ok(())
}
