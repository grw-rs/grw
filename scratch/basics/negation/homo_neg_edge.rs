use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- edge, Homo + !N_() (expect 2) ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(verbose &g, search![
        get(Homo) {
            N(0) ^ N(1)
                 ^ !N_()
        }
    ]);

    Ok(())
}
