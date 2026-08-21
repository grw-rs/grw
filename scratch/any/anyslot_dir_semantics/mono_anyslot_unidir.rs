use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Mono % on unidirectional 0->1 ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
    ]?;

    trace!(&g, search![
        get(Mono) {
            N(0) % N(1)
        }
    ]);

    Ok(())
}
