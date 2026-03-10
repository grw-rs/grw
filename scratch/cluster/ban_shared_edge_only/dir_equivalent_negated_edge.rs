use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Dir: equivalent negated edge form ---");

    let g: graph::Dir0 = graph![
        N(0) >> (N(1) >> N(2)),
        n(2) >> n(0)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> (N(1) >> N(2)),
            n(0) & !E() << n(2)
        }
    ]);

    Ok(())
}
