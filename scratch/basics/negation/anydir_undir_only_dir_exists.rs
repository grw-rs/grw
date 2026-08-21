use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- undir 0-1, only dir exists (expect 0) ---");

    let g: graph::Anydir0 = graph![
        N(0) >> (N(1) ^ N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        }
    ]);

    Ok(())
}
