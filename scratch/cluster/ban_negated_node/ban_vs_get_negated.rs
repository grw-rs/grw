use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Then what does the ban cluster actually check?
    // ban_shared_edges_satisfied: the edge n(1)^!N_() is negated.
    //   One endpoint (N_) is not in mapping (it's ban_only or negated).
    //   Skip (mapping[edge.source] or mapping[edge.target] returns None).
    // ban_only_nodes: empty (negated nodes excluded).
    // → ban_cluster_satisfiable returns true immediately (shared edges pass + no ban_only).
    // → match is REJECTED by ban... even though the negation didn't fire?
    //
    // That seems wrong. Let me verify:

    println!("--- ban with ONLY negated ban_only: what does ban check see? ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(1) ^ !N_()
        }
    ]);

    Ok(())
}
