use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Edge predicates inside ban clusters.
    //
    // Q: How do edge preds in ban work?
    // ban_shared_edges_satisfied checks pred on shared edges.
    // ban_node_feasible checks pred on ban_only edges.
    //
    // The ban is "reject if this pattern can be satisfied."
    // With a pred: "reject if this edge exists AND pred(val) is true."
    //
    // Example: get edge, ban "reject if matched pair has an edge with val > 10"
    // This is a conditional ban.

    println!("--- ban with edge pred: conditional rejection ---");

    let g: graph::UndirE<i32> = graph![
        N(0) & E().val(5) ^ N(1),
        n(0) & E().val(15) ^ N(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) & E().test(|v| *v > 10) ^ N(3)
        }
    ]);

    // Match {0→0,1→1}: ban needs N(3) adj to 0 with val>10. 0-2 has val 15>10. Ban fires. Rejected.
    // Match {0→0,1→2}: ban needs N(3) adj to 0 with val>10. Same. Rejected.
    // Match {0→1,1→0}: ban needs N(3) adj to 1 with val>10. 1-0 val=5. 1 has no other edge?
    //   Actually 1 is adj to 0 (val 5) only. No val>10 edge. Ban not satisfied. Survives.
    // Match {0→2,1→0}: ban needs N(3) adj to 2 with val>10. 2-0 val=15>10. Ban fires. Rejected.

    Ok(())
}
