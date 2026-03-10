use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Same on a complete graph (K4) — now everyone IS adjacent

    println!("--- ban negated edge on K4: no non-neighbor exists ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ N(2)
             ^ N(3),
        n(1) ^ n(2)
             ^ n(3),
        n(2) ^ n(3)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) & !E() ^ N(2)
        }
    ]);

    Ok(())
}
