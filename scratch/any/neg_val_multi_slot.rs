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
    println!("--- +% + !E().val(5)% on Anydir: valid (pred makes it non-contradictory) ---");

    expect_valid("+% + !E().val(5)% anydir", compile::<(), edge::Anydir<i32>>(search![
        get(Mono) {
            N(0) % N(1),
            n(0) & !E().val(5) % n(1)
        }
    ]));

    println!("--- +^ + !E().val(5)^ on Undir: valid (same reason) ---");

    expect_valid("+^ + !E().val(5)^ undir", compile::<(), edge::Undir<i32>>(search![
        get(Mono) {
            N(0) ^ N(1),
            n(0) & !E().val(5) ^ n(1)
        }
    ]));

    println!("--- +% + !% (no preds): truly contradictory ---");

    expect_reject("+% + !% no preds", compile::<(), edge::Anydir<i32>>(search![
        get(Mono) {
            N(0) % N(1),
            n(0) & !E() % n(1)
        }
    ]));

    println!("--- +% + !<< (no preds): not contradictory ---");

    expect_valid("+% + !<< no preds", compile::<(), edge::Anydir<i32>>(search![
        get(Mono) {
            N(0) % N(1),
            n(0) & !E() << n(1)
        }
    ]));

    Ok(())
}
