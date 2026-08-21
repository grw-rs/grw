use grw::*;

fn expect_valid<T, E: std::fmt::Debug>(label: &str, result: Result<T, E>) {
    match result {
        Ok(_) => println!("  {label}: OK - valid"),
        Err(e) => println!("  {label}: FAIL - expected valid, got: {e:?}"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Case 2: ban_only with edge to ONE shared node only.

    println!("--- not subsumed: single-edge ban_only ---");

    expect_valid("edge→extra neighbor", compile::<(), edge::Undir<()>>(search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(1) ^ N(3)
        }
    ]));

    println!("  runtime on path (no extra neighbor for endpoints):");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(1) ^ N(3)
        }
    ]);

    Ok(())
}
