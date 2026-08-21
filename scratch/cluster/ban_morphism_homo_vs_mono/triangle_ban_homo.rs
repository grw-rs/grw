use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- triangle: ban(Homo) common neighbor ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Homo) {
            n(0) ^ N(2),
            n(1) ^ n(2)
        }
    ]);

    Ok(())
}
