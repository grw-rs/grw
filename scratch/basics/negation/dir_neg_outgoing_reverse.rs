use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- neg outgoing 1->0, no edge (expect 1) ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> N(1),
            n(1) & !E() >> n(0)
        }
    ]);

    Ok(())
}
