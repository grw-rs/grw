use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Same but only 1 ban with 2 ban_only nodes connected to same shared node.
    // Within a single ban, Mono forces ban_only nodes to be distinct.
    // Q: So a single ban(Mono) { n(1) ^ N(5), n(1) ^ N(6) } requires
    //    TWO distinct neighbors of matched[1]. Correct?

    println!("--- single ban, 2 ban_only nodes need distinct targets ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(1) ^ N(5)
                 ^ N(6)
        }
    ]);

    Ok(())
}
