use grw::*;

fn expect_valid<T, E: std::fmt::Debug>(label: &str, result: Result<T, E>) {
    match result {
        Ok(_) => println!("  {label}: OK - valid"),
        Err(e) => println!("  {label}: FAIL - expected valid, got: {e:?}"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Case 3: ban defines new node not in get.

    println!("--- ban defines new node not in get ---");

    expect_valid("ban(Mono) { N(5) ^ n(0) }", compile::<(), edge::Undir<()>>(search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            N(5) ^ n(0)
        }
    ]));

    println!("  runtime on path:");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            N(5) ^ n(0)
        }
    ]);

    Ok(())
}
