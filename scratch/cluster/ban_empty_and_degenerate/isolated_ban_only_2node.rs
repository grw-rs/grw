use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Case 2: Ban with only ban_only nodes, no edges.
    // ban(Mono) { N(2) } — on 2-node target: both mapped, no candidate → survives.
    //                       on 3-node target: unmapped node exists → ban fires → rejected.

    println!("--- ban(Mono) isolated ban_only: 2-node target ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            N(2)
        }
    ]);

    Ok(())
}
