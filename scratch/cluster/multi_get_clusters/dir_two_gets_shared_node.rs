use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Dir graph with two get clusters using different directions.
    // get(Mono) { N(0)>>N(1) }, get(Mono) { n(0)<<N(2) }
    // Pattern: 0 has outgoing edge to 1 AND incoming edge from 2.
    // These are different slots (>> and <<).

    println!("--- Dir: two gets with >> and << on shared node ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1),
        N(2) >> n(0),
        N(3) >> n(0)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(10) >> N(11)
        },
        get(Mono) {
            n(10) << N(12)
        }
    ]);

    Ok(())
}
