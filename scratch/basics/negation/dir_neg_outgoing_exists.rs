use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- neg outgoing 0->1, edge exists (expect 0) ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> N(1),
            n(0) & !E() >> n(1)
        }
    ]);

    Ok(())
}
