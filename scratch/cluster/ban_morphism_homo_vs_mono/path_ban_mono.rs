use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Now a graph where the difference matters: path 0-1-2 (no triangle)
    // get: edge (Mono) → 4 matches
    // ban: "common neighbor of matched pair"
    //
    // ban(Mono): N(2) must be ∉ {a,b}. For match {0→0,1→1}: need c ∉ {0,1} adj to both.
    //   No such c exists (only 2 is adj to 1, not to 0). → match survives
    //
    // ban(Homo): N(2) can be a or b. For match {0→0,1→1}: can N(2)→0? adj(0,0)?
    //   No self-loop. N(2)→1? adj(1,0)=yes AND adj(1,1)=no self-loop. Nope.
    //   So match survives too... unless target has self-loops.
    //
    // Q: Is there any graph where ban(Homo) filters MORE than ban(Mono)?

    println!("--- path: ban(Mono) common neighbor ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) ^ N(2),
            n(1) ^ n(2)
        }
    ]);

    Ok(())
}
