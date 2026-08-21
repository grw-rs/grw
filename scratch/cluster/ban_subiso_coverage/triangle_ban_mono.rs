use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- ban(SubIso) vs ban(Mono) on triangle ---");

    println!("  ban(Mono):");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(1) ^ N(3)
        }
    ]);

    Ok(())
}
