use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ban shared REVERSE slot: different edge, NOT subsumed.

    println!("--- Dir: ban shared REVERSE slot → survives when no reverse edge ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> N(1)
        },
        ban(Mono) {
            n(0) << n(1)
        }
    ]);

    Ok(())
}
