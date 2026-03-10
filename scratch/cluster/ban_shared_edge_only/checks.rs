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
    println!("--- Dir: ban shared same-slot as get → RedundantInBan ---");

    expect_reject("ban shared same-slot", compile::<(), edge::Dir<()>>(search![
        get(Mono) {
            N(0) >> N(1)
        },
        ban(Mono) {
            n(0) >> n(1)
        }
    ]));

    println!("--- Anydir: ban % between shared (get has >>) → OK (different slot) ---");

    expect_valid("ban % with >> in get", compile::<(), edge::Anydir<()>>(search![
        get(Mono) {
            N(0) >> N(1)
        },
        ban(Mono) {
            n(0) % n(1)
        }
    ]));

    println!("--- Anydir: ban % between shared (get has ^) → OK (different slot) ---");

    expect_valid("ban % with ^ in get", compile::<(), edge::Anydir<()>>(search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) % n(1)
        }
    ]));

    Ok(())
}
