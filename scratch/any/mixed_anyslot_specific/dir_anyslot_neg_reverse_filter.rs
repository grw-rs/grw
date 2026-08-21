use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Dir 0->1: % + !<< filters to forward-only ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
    ]?;

    trace!(&g, search![
        get(Mono) {
            N(0) % N(1)
                 & !E() << n(1)
        }
    ]);

    Ok(())
}
