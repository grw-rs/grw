use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // When a node appears in multiple get clusters with different morphisms,
    // it gets min(morphisms) — the MOST restrictive.
    //
    // Q: Is this always correct? Does the min-morphism apply globally,
    //    or should each cluster enforce its own morphism independently?

    // Node 10 in Iso cluster (exact degree), node 10 also in Mono cluster.
    // min(Iso, Mono) = Iso → node 10 gets Iso degree constraint.
    // But the Mono cluster only needs Mono-level filtering.
    // Currently: node 10 is filtered as Iso everywhere.

    println!("--- mixed Iso+Mono get: shared node gets Iso ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ (N(2) ^ N(3)))
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
