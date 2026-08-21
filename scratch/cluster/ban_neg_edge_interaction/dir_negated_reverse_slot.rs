use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Dir version: ban negated >> between shared nodes.
    // get: 0 >> 1. ban: n(0) & !E() >> n(1).
    // Get guarantees 0>>1 exists. Ban says "reject if 0>>1 NOT exists" → never fires.
    // But what about ban { n(0) & !E() << n(1) }? Different slot!
    // Get guarantees >> slot exists, ban checks << slot doesn't exist.
    // If only 0→1 (no 1→0), ban finds << absent → ban IS satisfied → match rejected!

    println!("--- Dir: ban negated reverse-slot shared edge ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> N(1)
        },
        ban(Mono) {
            n(0) & !E() << n(1)
        }
    ]);

    Ok(())
}
