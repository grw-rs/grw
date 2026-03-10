use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Two get clusters, same nodes, same edges, different morphisms.
    // get(Iso) { N(0)^N(1) }, get(Homo) { n(0)^n(1) }
    // Both contribute edges to the same pair. But edge dedup catches this.
    // Actually: it's the SAME edge (same lo, hi, negated=false, slot).
    // Second cluster's flatten: edge_map finds existing entry.
    // cluster_edges gets the existing index pushed. No error (cross-cluster sharing).
    // Node 0 gets min(Iso, Homo) = Iso.
    // Node 1 gets min(Iso, Homo) = Iso.
    // So the Homo cluster is effectively overridden by the stricter Iso.

    println!("--- two gets same edge different morphisms ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(verbose &g, search![
        get(Iso) {
            N(0) ^ N(1)
        },
        get(Homo) {
            n(0) ^ n(1)
        }
    ]);

    Ok(())
}
