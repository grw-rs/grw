use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Two get clusters referencing each other's nodes.
    // get(Iso) { N(0)^N(1) }, get(Mono) { n(0)^N(2) }
    // Node 0 is in both clusters. Gets min(Iso, Mono) = Iso.
    // Node 1: only in Iso cluster → Iso.
    // Node 2: only in Mono cluster → Mono.
    //
    // But the degree check for node 0 under Iso considers ALL edges
    // (from both clusters). Pattern degree of 0 = 2 (edges to 1 and 2).
    // So target node for 0 must have EXACTLY degree 2.

    println!("--- overlapping get clusters: node in both ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ N(2),
        n(1) ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(Iso) {
            N(10) ^ N(11)
        },
        get(Mono) {
            n(10) ^ N(12)
        }
    ]);

    Ok(())
}
