use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Hmm, but this is the EXISTING test: neg_multi_cluster.
    // That test says 0 matches on a triangle. Let me check path.

    println!("--- ban with negated ban_only on path ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(1) ^ !N_()
        }
    ]);

    Ok(())
}
