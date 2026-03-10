use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Wait: get { N(0) << N(1) } means N(0) is the RECEIVER.
    // Graph has 0→1 (edge source=0, target=1, slot=>>).
    // Pattern N(0)<<N(1) means: source=N(1), target=N(0), slot=>>?
    // Or does << mean reverse_slot?
    // In the DSL: A << B creates EdgeOp with slot=reverse(>>), any_slot=false.
    // For Dir: reverse_slot(Fwd) = Bwd, reverse_slot(Bwd) = Fwd.
    // So N(0) << N(1): N(0) has edge to N(1) with slot Bwd.
    // After normalization: edge (0→1 with slot Bwd) or (1→0 with slot Fwd)?
    // In the graph: edge is (0,1,Fwd). Pattern asks for (0,1,Bwd)?
    // That means "0←1" which is "1→0". Graph only has 0→1, not 1→0.
    // So match {N(0)→1, N(1)→0}? 1 points to 0... no, graph has 0→1.
    // Hmm, need to trace carefully.

    println!("--- Dir: who matches N(0) << N(1) ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) << N(1)
        }
    ]);

    Ok(())
}
