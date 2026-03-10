use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Dir: ban shared edge with different slot than get.
    // get: 0>>1, 1>>2. ban: n(2)>>n(0). "reject if 2→0 exists"

    println!("--- Dir: ban shared edge, different slot than get ---");

    let g: graph::Dir0 = graph![
        N(0) >> (N(1) >> N(2)),
        n(2) >> n(0)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> (N(1) >> N(2))
        },
        ban(Mono) {
            n(2) >> n(0)
        }
    ]);

    Ok(())
}
