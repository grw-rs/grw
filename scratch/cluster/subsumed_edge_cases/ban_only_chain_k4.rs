use grw::*;

fn expect_valid<T, E: std::fmt::Debug>(label: &str, result: Result<T, E>) {
    match result {
        Ok(_) => println!("  {label}: OK - valid"),
        Err(e) => println!("  {label}: FAIL - expected valid, got: {e:?}"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Case 3: ban with edge between ban_only nodes.

    println!("--- skipped: ban_only nodes connected to each other ---");

    expect_valid("ban_only chain", compile::<(), edge::Undir<()>>(search![
        get(Mono) {
            N(0) ^ (N(1) ^ N(2))
                 ^ n(2)
        },
        ban(Mono) {
            n(0) ^ (N(3) ^ (N(4) ^ n(1)))
        }
    ]));

    println!("  runtime on K4:");

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
            N(0) ^ (N(1) ^ N(2))
                 ^ n(2)
        },
        ban(Mono) {
            n(0) ^ (N(3) ^ (N(4) ^ n(1)))
        }
    ]);

    Ok(())
}
