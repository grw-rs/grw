use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: What if Iso cluster requires degree=1 but Mono cluster
    //    adds another edge to the same node, making required degree=2?
    // Node 10 has edges in both clusters: 10-12 (Iso) and 10-11 (Mono).
    // Compiled pattern: node 10 has degree 2 (two positive neighbors).
    // Under Iso: target node for 10 must have EXACTLY degree 2.
    // Under Mono: target node for 10 must have AT LEAST degree 2.
    // min(Iso, Mono) = Iso → requires exact degree 2.
    //
    // But wait: if we Iso-match node 10 with degree 2, then SubIso coverage
    // check ensures ALL edges of that target node are accounted for.
    // Is this what we want from "get(Iso) { N(10) ^ N(12) }"?
    // The Iso cluster only has 1 edge for node 10, but another cluster
    // adds a 2nd. The combined pattern has degree 2 for node 10.

    // Diamond: 0-1, 0-2, 1-3, 2-3 (degree 2 everywhere)

    println!("--- diamond graph: Iso+Mono shared node ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ N(2),
        n(1) ^ N(3),
        n(2) ^ n(3)
    ]?;

    trace!(verbose &g, search![
        get(Iso) {
            N(10) ^ N(12)
        },
        get(Mono) {
            n(10) ^ N(11)
        }
    ]);

    Ok(())
}
