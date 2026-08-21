use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- compare to get-only with negated ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
                 ^ !N_()
        }
    ]);

    Ok(())
}
