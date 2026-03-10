use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Ban with ONLY negated edges to ban_only node.
    // ban { n(0) & !E() ^ N(2) }
    // "ban if ∃ node c NOT adjacent to matched[0]"
    // In almost any graph with >1 node, there exist non-neighbors.
    // This is almost always satisfiable → basically rejects everything.
    // Should this be caught at compile time?

    println!("--- ban with only negated edges: always satisfiable? ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) & !E() ^ N(2)
        }
    ]);

    Ok(())
}
