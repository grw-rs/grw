use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: get(Homo) with 3-node pattern allowing collapse.
    // Pattern: N(0)^N(1), n(1)^N(2). Under Homo, N(0) and N(2) can map same.
    // Target: single edge 0-1.
    // {0→0,1→1,2→0}: need 0-1 edge ✓, 1-0 edge ✓. Match!
    // {0→1,1→0,2→1}: need 1-0 ✓, 0-1 ✓. Match!
    // What about {0→0,1→0,2→0}? need 0-0 edge (self-loop). No. Infeasible.
    // So: Homo path pattern on single edge → collapses endpoints.

    println!("--- Homo path pattern collapsing on single edge ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(verbose &g, search![
        get(Homo) {
            N(0) ^ (N(1) ^ N(2))
        }
    ]);

    Ok(())
}
