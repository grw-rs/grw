use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Anydir: ban with specific slot (>>) when get uses (^).
    // get: N(0)^N(1) (undirected). ban: n(0)>>N(2).
    // "reject if matched[0] has a directed outgoing edge to some node"

    println!("--- Anydir: get ^ ban >> ---");

    let g: graph::Anydir0 = graph![
        N(0) ^ N(1)
             >> N(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) >> N(2)
        }
    ]);

    Ok(())
}
