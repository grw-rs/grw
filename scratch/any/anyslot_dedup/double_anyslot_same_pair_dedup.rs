use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- double % same pair: deduplicates ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(&g, search![
        get(Mono) {
            N(0) % N(1)
                 % n(1)
        }
    ]);

    Ok(())
}
