use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Multiple get clusters define a SINGLE combined pattern.
    // All get clusters are flattened into one node/edge set.
    // The only per-cluster effect is morphism assignment.
    //
    // Q: What does it mean to have two get clusters with different morphisms
    //    that share NO nodes?

    // get(Iso) { N(0)^N(1) }, get(Mono) { N(2)^N(3) }
    // Node 0,1 get Iso morphism. Node 2,3 get Mono.
    // The search_order has all 4 nodes.
    // Target must be matched: Iso nodes need exact degree, Mono nodes need min degree.
    // But they're in different connected components of the pattern.
    // So matches are cross-products of Iso matches × Mono matches?

    println!("--- disconnected get clusters: Iso + Mono ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2),
        N(3) ^ N(4)
    ]?;

    trace!(verbose &g, search![
        get(Iso) {
            N(10) ^ N(11)
        },
        get(Mono) {
            N(12) ^ N(13)
        }
    ]);

    Ok(())
}
