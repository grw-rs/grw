use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- ban with negated edge to ban_only: path (endpoint has unshared neighbor) ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) ^ N(2),
            n(2) & !E() ^ n(1)
        }
    ]);

    Ok(())
}
