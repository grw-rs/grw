use grw::*;

fn expect_reject<T, E: std::fmt::Display>(label: &str, result: Result<T, E>) {
    match result {
        Err(e) => println!("  {label}: OK - rejected ({e})"),
        Ok(_) => println!("  {label}: FAIL - expected rejection, compiled"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Case 1: Ban referencing only shared nodes, no edges at all.
    // ban(Mono) { n(0) } → Subsumed (ban always satisfiable, no constraints)

    println!("--- ban with shared node only, no edges → Subsumed ---");

    expect_reject("ban(Mono) { n(0) }", compile::<(), edge::Undir<()>>(search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0)
        }
    ]));

    // Case 4: Multiple bans, first is trivial (shared node only → Subsumed).

    println!("--- multiple bans, first is trivial → Subsumed ---");

    expect_reject("ban trivial + ban real", compile::<(), edge::Undir<()>>(search![
        get(Mono) {
            N(0) ^ (N(1) ^ N(2))
                 ^ n(2)
        },
        ban(Mono) {
            n(0)
        },
        ban(Mono) {
            n(0) ^ N(5)
                 ^ n(1)
        }
    ]));

    Ok(())
}
