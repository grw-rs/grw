use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ban with only shared nodes and a NEW edge between them.
    // No ban_only nodes. The ban says: "reject if this edge
    // exists between two already-matched nodes."
    //
    // Q: Equivalent to negated edge in get?

    println!("--- ban shared-only: triangle rejected ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ (N(1) ^ N(2))
        },
        ban(Mono) {
            n(0) ^ n(2)
        }
    ]);

    Ok(())
}
