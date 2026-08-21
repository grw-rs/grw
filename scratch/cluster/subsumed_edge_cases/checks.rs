use grw::*;

fn expect_valid<T, E: std::fmt::Debug>(label: &str, result: Result<T, E>) {
    match result {
        Ok(_) => println!("  {label}: OK - valid"),
        Err(e) => println!("  {label}: FAIL - expected valid, got: {e:?}"),
    }
}

fn expect_reject<T, E: std::fmt::Display>(label: &str, result: Result<T, E>) {
    match result {
        Err(e) => println!("  {label}: OK - rejected ({e})"),
        Ok(_) => println!("  {label}: FAIL - expected rejection, compiled"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    expect_valid("triangle ban(Mono)", compile::<(), edge::Undir<()>>(search![
        get(Mono) {
            N(0) ^ (N(1) ^ N(2))
                 ^ n(2)
        },
        ban(Homo) {
            n(0) ^ N(3)
                 ^ n(1)
        }
    ]));

    // Case 4: ban with any_slot edge (% — skipped by subsumed check).

    println!("--- skipped: any_slot in ban ---");

    expect_valid("% edge ban", compile::<(), edge::Undir<()>>(search![
        get(Mono) {
            N(0) ^ (N(1) ^ N(2))
                 ^ n(2)
        },
        ban(Mono) {
            n(0) % (N(3) % n(1))
        }
    ]));

    expect_reject("ban(Homo) >>chain", compile::<(), edge::Dir<()>>(search![
        get(Mono) {
            N(0) >> (N(1) >> N(2))
        },
        ban(Homo) {
            n(0) >> (n(1) >> N_())
        }
    ]));

    // Case 7: empty-ish ban (shared node only, no edges → Subsumed).

    println!("--- degenerate: ban with shared node only ---");

    expect_reject("empty-ish ban", compile::<(), edge::Undir<()>>(search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0)
        }
    ]));

    Ok(())
}
