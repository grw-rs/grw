use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- % + ^ same pair: different keys, both applied ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(&g, search![
        get(Mono) {
            N(0) % N(1)
                 ^ n(1)
        }
    ]);

    Ok(())
}
