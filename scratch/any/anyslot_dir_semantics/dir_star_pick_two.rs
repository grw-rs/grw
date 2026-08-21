use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Dir star: 0->1, 0->2, 0->3; pick 2 via % ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
             >> N(2)
             >> N(3)
    ]?;

    trace!(&g, search![
        get(Mono) {
            N(0) % N(1)
                 % N(2)
        }
    ]);

    Ok(())
}
