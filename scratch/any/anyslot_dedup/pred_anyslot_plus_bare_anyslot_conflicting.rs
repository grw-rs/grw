use grw::*;

fn expect_reject<T, E: std::fmt::Display>(label: &str, result: Result<T, E>) {
    match result {
        Err(e) => println!("  {label}: OK - rejected ({e})"),
        Ok(_) => println!("  {label}: FAIL - expected rejection, compiled"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- pred % + bare %: ConflictingPred ---");

    expect_reject("pred % + bare %", compile::<(), edge::Undir<i32>>(search![
        get(Mono) {
            N(0) & E().val(5) % N(1)
                              % n(1)
        }
    ]));

    Ok(())
}
