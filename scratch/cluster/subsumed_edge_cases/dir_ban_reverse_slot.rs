use grw::*;

fn expect_valid<T, E: std::fmt::Debug>(label: &str, result: Result<T, E>) {
    match result {
        Ok(_) => println!("  {label}: OK - valid"),
        Err(e) => println!("  {label}: FAIL - expected valid, got: {e:?}"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Case 6: Dir ban reverse slot.
    // ban: n(1) << N_() — "reject if something points TO matched[1]"

    println!("--- Dir: ban << where get only has >> ---");

    expect_valid("ban reverse", compile::<(), edge::Dir<()>>(search![
        get(Mono) {
            N(0) >> (N(1) >> N(2))
        },
        ban(Mono) {
            n(1) << N_()
        }
    ]));

    println!("  runtime on 0→1→2 (0 points to 1):");

    let g: graph::Dir0 = graph![
        N(0) >> (N(1) >> N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> (N(1) >> N(2))
        },
        ban(Mono) {
            n(1) << N_()
        }
    ]);

    Ok(())
}
