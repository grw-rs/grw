use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // What if target has BOTH >> and ^ between same pair?

    println!("--- Anydir: target has >> AND ^ between same pair ---");

    let g: graph::Anydir0 = graph![
        N(0) >> N(1)
             ^ n(1)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> N(1)
        },
        ban(Mono) {
            n(0) ^ n(1)
        }
    ]);

    Ok(())
}
