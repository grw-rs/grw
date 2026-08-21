use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- single, Iso edge (expect 1) ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
    ]?;

    trace!(verbose &g, search![
        get(Iso) {
            N(0) >> N(1)
        }
    ]);

    Ok(())
}
