use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ban(Mono) { n(10) ^ n(11) } — "reject if the two matched nodes are adjacent"

    println!("--- get disconnected + ban adjacent ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        N(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(10), N(11)
        },
        ban(Mono) {
            n(10) ^ n(11)
        }
    ]);

    Ok(())
}
