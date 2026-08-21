use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- undir 1-2 + !dir 1->2 (expect 2) ---");

    let g: graph::Anydir0 = graph![
        N(0) >> (N(1) ^ N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(10) ^ N(11),
            n(10) & !E() >> n(11)
        }
    ]);

    Ok(())
}
