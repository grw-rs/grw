use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Dir chain 0->1->2 via % ---");

    let g: graph::Dir0 = graph![
        N(0) >> (N(1) >> N(2))
    ]?;

    trace!(&g, search![
        get(Mono) {
            N(0) % (N(1) % N(2))
        }
    ]);

    Ok(())
}
