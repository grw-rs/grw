use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Anydir ban with ^ on shared when get has >>.
    // ^ is undir slot. >> is dir slot. Different slots.
    // If only >> exists between matched pair, ^ is NOT satisfied.
    // ban doesn't fire.

    println!("--- Anydir: ban ^ on shared with >> in get ---");

    let g: graph::Anydir0 = graph![
        N(0) >> N(1)
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
