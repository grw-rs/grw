use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ban cluster with negated edge to ban_only node.
    // "Ban any match where n(0) has a neighbor that is NOT connected to n(1)"
    //
    // Q: What does it mean for a ban to have a negated edge to a ban_only node?
    // The ban_backtrack checks is_feasible-like logic per edge.
    // A negated edge means "this edge must NOT exist" for the ban to be satisfiable.
    // So ban_node_feasible returns false if the negated edge IS present.
    //
    // Effectively: ban { n(0) ^ N(2), n(2) & !E() ^ n(1) }
    //   = "ban if ∃ node c: c adj 0, c NOT adj 1"
    //   = "reject match if matched[0] has a neighbor not adjacent to matched[1]"

    println!("--- ban with negated edge to ban_only: triangle (all neighbors shared) ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) ^ N(2),
            n(2) & !E() ^ n(1)
        }
    ]);

    Ok(())
}
