use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Multiple ban clusters: each is checked independently.
    // Q: Is there a case where ban1 alone passes, ban2 alone passes,
    //    but together they should fail because the ban_only_nodes
    //    in ban1 and ban2 would need to share the same target node?
    //
    // Currently: bans are checked independently with separate ban_mappings.
    // No cross-ban injectivity constraint.

    // target: 0-1-2-3 path + 1-3 shortcut
    //   0 - 1 - 2 - 3
    //       |       |
    //       +-------+
    //
    // get: edge 0-1 (Mono)
    // ban1: n(0) ^ N(5) ^ n(1)  (common neighbor of matched pair)
    // ban2: n(1) ^ N(6)         (matched[1] has extra neighbor)
    //
    // For match {0→1, 1→0}: ban1 needs N(5) adj to 1 and 0 → no 3rd node adj both
    //   Actually 2 adj to 1, not adj to 0 (no 0-2 edge). So ban1 NOT satisfied → passes ban1.
    //   ban2: N(6) adj to 0. Nodes adj to 0: just 1. Under Mono, 1 is mapped. No candidate. → passes.
    //   Match survives both bans independently.
    //
    // For match {0→1, 1→2}: ban1: N(5) adj 1 and 2, not in {1,2}. 0 adj 1 yes, adj 2 no. 3 adj 2 yes, adj 1 yes! → N(5)→3 works. Banned.

    println!("--- multi-ban: independent checking ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2)
                     ^ N(3)),
        n(2) ^ n(3)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) ^ N(5),
            n(1) ^ n(5)
        },
        ban(Mono) {
            n(1) ^ N(6)
        }
    ]);

    Ok(())
}
