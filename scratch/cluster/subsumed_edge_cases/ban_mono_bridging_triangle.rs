use grw::*;

fn expect_valid<T, E: std::fmt::Debug>(label: &str, result: Result<T, E>) {
    match result {
        Ok(_) => println!("  {label}: OK - valid"),
        Err(e) => println!("  {label}: FAIL - expected valid, got: {e:?}"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Case 1: ban_only bridging two get nodes in triangle.
    // Under Mono: ban_only needs fresh node → not subsumed.
    // Under Homo: ban_only can reuse get node → subsumed IF structurally guaranteed.
    // (Now skipped for injective ban)

    println!("--- ban(Mono) ban_only bridging two get nodes ---");

    expect_valid("triangle ban(Mono)", compile::<(), edge::Undir<()>>(search![
        get(Mono) {
            N(0) ^ (N(1) ^ N(2))
                 ^ n(2)
        },
        ban(Mono) {
            n(0) ^ N(3)
                 ^ n(1)
        }
    ]));

    println!("  runtime on triangle:");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ (N(1) ^ N(2))
                 ^ n(2)
        },
        ban(Mono) {
            n(0) ^ N(3)
                 ^ n(1)
        }
    ]);

    Ok(())
}
