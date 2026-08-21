use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- triangle, Homo + !N_() (expect 0) ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(Homo) {
            N(0) ^ N(1)
                 ^ !N_()
        }
    ]);

    Ok(())
}
