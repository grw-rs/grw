use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Two get clusters, both Mono, sharing a node.
    //    The node's degree is the UNION of edges from both clusters.
    //    Is the degree filter computed from the combined adj, or per-cluster?

    println!("--- two Mono gets sharing node: path coverage ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ (N(2) ^ N(3)))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(10) ^ N(11)
        },
        get(Mono) {
            n(11) ^ N(12)
        }
    ]);

    Ok(())
}
