use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Negated nodes inside ban clusters.
    //
    // Q: How does a negated node inside a ban interact with the ban logic?
    // A negated node in a get cluster means "this node must NOT exist with these edges."
    // Inside a ban: ???
    //
    // compile.rs: ban clusters with negated nodes skip the subsumed check.
    // backtrack: ban_cluster_satisfiable → ban_shared_edges_satisfied → ban_backtrack.
    // ban_backtrack iterates over ban_only_nodes.
    // A negated node in ban_only_nodes: it's still in the backtrack loop.
    //
    // But wait: compile.rs marks edges as negated when source OR target is negated.
    // So if ban has { n(0) ^ !N_() }, the edge is negated (because target is negated).
    // In ban_node_feasible, a negated edge means "must NOT exist for ban to be satisfiable."
    //
    // So ban { n(0) ^ !N(2) } with N(2) being ban_only means:
    //   "ban is satisfiable if ∃ node c such that c is NOT adjacent to matched[0]"
    //   = "reject match if any non-neighbor of matched[0] exists"
    //   This rejects almost everything (unless target is complete graph).

    println!("--- ban with negated ban_only node: rejects unless complete ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
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
