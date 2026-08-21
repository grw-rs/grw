use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- dir 0->1 (expect 1) ---");

    let g: graph::Anydir0 = graph![
        N(0) >> (N(1) ^ N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> N(1)
        }
    ]);

    Ok(())
}
