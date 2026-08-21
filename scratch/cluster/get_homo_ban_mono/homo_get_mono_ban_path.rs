use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: get(Homo) + ban(Mono) on a path.
    // get(Homo) { N(0)^N(1) } matches 2 assignments on 0-1 edge.
    // ban(Mono) { n(0)^N(2)^n(1) } needs distinct 3rd node adj both.
    // On path 0-1-2: for {0→0,1→1}: need c∉{0,1} adj(c,0) adj(c,1). No such c. Survives.
    //                for {0→1,1→0}: need c∉{1,0} adj(c,1) adj(c,0). Same. Survives.
    // Both survive → 2 matches.

    println!("--- Homo get + Mono ban: path ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
    ]?;

    trace!(verbose &g, search![
        get(Homo) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) ^ N(2),
            n(1) ^ n(2)
        }
    ]);

    Ok(())
}
