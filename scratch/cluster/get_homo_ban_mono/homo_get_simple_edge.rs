use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: get(Homo) allows multiple pattern nodes → same target node.
    //    ban(Mono) requires ban_only nodes to be distinct from get-mapped.
    //    But under Homo, the reverse map might have collisions.
    //    How does ban injectivity interact with Homo get?

    // target: single edge 0-1
    // get(Homo): N(0)^N(1) → matches include non-injective ones like {0→0,1→0}?
    // Wait — Homo + edge requires adjacency. is_adjacent(0,0) = self-loop check.
    // No self-loop → {0→0,1→0} infeasible. Only standard {0→0,1→1} and {0→1,1→0}.
    // So Homo on a simple edge is same as Mono. Need self-loops or multi-edges.

    println!("--- Homo get on simple edge (no self-loops) ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(verbose &g, search![
        get(Homo) {
            N(0) ^ N(1)
        }
    ]);

    Ok(())
}
