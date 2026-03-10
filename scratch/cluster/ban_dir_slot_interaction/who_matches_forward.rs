use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Dir: who matches N(0) >> N(1) ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> N(1)
        }
    ]);

    Ok(())
}
