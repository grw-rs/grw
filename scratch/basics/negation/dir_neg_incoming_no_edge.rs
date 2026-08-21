use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- neg incoming 1->0, no edge (expect 1) ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) << N(1),
            n(0) & !E() << n(1)
        }
    ]);

    Ok(())
}
