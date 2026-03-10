use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Trace a few interesting combos for visual inspection:

    println!("--- diamond: get(Mono) + ban(Mono) common neighbor ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ N(2)
             ^ N(3),
        n(1) ^ n(2)
             ^ n(3)
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
