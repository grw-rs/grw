use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // C4 cycle: 0-1, 1-2, 2-3, 3-0. Common neighbor exists only for adjacent pairs.

    println!("--- C4: get(Mono) + ban(Mono) common neighbor ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ (N(2) ^ N(3))),
        n(3) ^ n(0)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(10) ^ N(11)
        },
        ban(Mono) {
            n(10) ^ (N(12) ^ n(11))
        }
    ]);

    Ok(())
}
