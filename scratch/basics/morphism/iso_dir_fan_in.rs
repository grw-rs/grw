use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- fan-in, Iso edge (expect 0) ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1),
        N(2) >> n(1)
    ]?;

    trace!(verbose &g, search![
        get(Iso) {
            N(0) >> N(1)
        }
    ]);

    Ok(())
}
