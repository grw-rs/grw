use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- bidir, Iso edge (expect 0) ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1),
        n(1) >> n(0)
    ]?;

    trace!(verbose &g, search![
        get(Iso) {
            N(0) >> N(1)
        }
    ]);

    Ok(())
}
