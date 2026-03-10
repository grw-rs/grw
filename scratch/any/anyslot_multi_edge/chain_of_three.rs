use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- chain of 3 via %: 0-1-2 path ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
    ]?;

    trace!(&g, search![
        get(Mono) {
            N(0) % (N(1) % N(2))
        }
    ]);

    Ok(())
}
