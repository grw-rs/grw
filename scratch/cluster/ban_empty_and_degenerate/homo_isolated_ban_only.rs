use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Case 5: ban(Homo) { N(2) } — Homo ban_only can map to ANY target node.
    // If target has ≥ 1 node, ban always fires → rejected.

    println!("--- ban(Homo) isolated ban_only: always fires ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Homo) {
            N(2)
        }
    ]);

    Ok(())
}
