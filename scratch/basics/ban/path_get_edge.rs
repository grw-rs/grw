use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- path: get edge (expect 6) ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ (N(2) ^ N(3)))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        }
    ]);

    Ok(())
}
