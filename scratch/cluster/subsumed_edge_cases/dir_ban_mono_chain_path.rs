use grw::*;

fn expect_valid<T, E: std::fmt::Debug>(label: &str, result: Result<T, E>) {
    match result {
        Ok(_) => println!("  {label}: OK - valid"),
        Err(e) => println!("  {label}: FAIL - expected valid, got: {e:?}"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Case 5: Dir ban with same slot as get chain.
    // get: 0>>1>>2. ban: n(0)>>n(1)>>N_().
    // Under Mono: N_() can't reuse N(2)'s target → not subsumed.
    // Under Homo: subsumed.

    println!("--- Dir: ban(Mono) chain same slot ---");

    expect_valid("ban(Mono) >>chain", compile::<(), edge::Dir<()>>(search![
        get(Mono) {
            N(0) >> (N(1) >> N(2))
        },
        ban(Mono) {
            n(0) >> (n(1) >> N_())
        }
    ]));

    println!("  runtime on path 0→1→2 (no extra outgoing from 1):");

    let g: graph::Dir0 = graph![
        N(0) >> (N(1) >> N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> (N(1) >> N(2))
        },
        ban(Mono) {
            n(0) >> (n(1) >> N_())
        }
    ]);

    Ok(())
}
