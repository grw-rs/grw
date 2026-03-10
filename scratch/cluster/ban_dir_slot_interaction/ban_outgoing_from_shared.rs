use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Ban ban_only node with >> on dir graph.
    // get: N(0)>>N(1). ban: n(1)>>N(2).
    // "reject if matched[1] has an outgoing edge to some other node"
    // On graph 0→1→2: match {0→0,1→1}. Ban: does 1 have outgoing? 1→2 exists. → rejected.

    println!("--- Dir: ban outgoing from shared ---");

    let g: graph::Dir0 = graph![
        N(0) >> (N(1) >> N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> N(1)
        },
        ban(Mono) {
            n(1) >> N(2)
        }
    ]);

    Ok(())
}
