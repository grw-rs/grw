use grw::*;

fn expect_valid<T, E: std::fmt::Debug>(label: &str, result: Result<T, E>) {
    match result {
        Ok(_) => println!("  {label}: OK - valid"),
        Err(e) => println!("  {label}: FAIL - expected valid, got: {e:?}"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- self-loop % compiles ---");

    expect_valid("self-loop %", compile::<(), edge::Undir<()>>(search![
        get(Mono) {
            N(0) % n(0)
        }
    ]));

    Ok(())
}
