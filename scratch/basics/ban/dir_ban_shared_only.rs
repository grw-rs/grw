use grw::*;

fn expect_reject<T, E: std::fmt::Display>(label: &str, result: Result<T, E>) {
    match result {
        Err(e) => println!("  {label}: OK - rejected ({e})"),
        Ok(_) => println!("  {label}: FAIL - expected rejection, compiled"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- dir: ban shared only (subsumed) ---");

    expect_reject("ban shared only", compile::<(), edge::Dir<()>>(search![
        get(Mono) {
            N(0) >> N(1)
        },
        ban(Mono) {
            n(0)
        }
    ]));

    Ok(())
}
