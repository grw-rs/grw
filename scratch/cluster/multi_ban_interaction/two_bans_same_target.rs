use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Two bans with ban_only_nodes that structurally require the same
    //    target node, but since bans are independent, each finds it separately.
    //    Is this correct? Or should cross-ban uniqueness be enforced?
    //
    // Example: get edge 0-1, target has exactly one extra node 2 adj to 1.
    // ban1: n(1) ^ N(5)  (node 1 has extra neighbor → N(5)→2)
    // ban2: n(1) ^ N(6)  (node 1 has extra neighbor → N(6)→2)
    // Both independently find N→2. Match is rejected twice over.
    // But if we required cross-ban injectivity (N(5)≠N(6)),
    // we'd need TWO distinct extra neighbors — only 1 exists.
    // The match would survive.
    //
    // Current behavior: both bans fire independently. Match is rejected.
    // Is this semantically right? Bans are constraints, not existential demands.

    println!("--- two bans both map ban_only to same target ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(1) ^ N(5)
        },
        ban(Mono) {
            n(1) ^ N(6)
        }
    ]);

    Ok(())
}
