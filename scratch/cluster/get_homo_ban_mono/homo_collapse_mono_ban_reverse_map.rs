use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Now add ban(Mono): n(0)^N(3)^n(2).
    // For match {0→0,1→1,2→0}: ban needs c∉{0,1,0}={0,1} adj to both 0 and 0.
    // adj(c,0) for c∉{0,1}: only node 2 if exists... target only has 0,1. → no c. Survives.
    // For match {0→1,1→0,2→1}: ban needs c∉{1,0,1}={0,1} adj 1 and 1.
    // Same issue. Survives.
    //
    // Q: The reverse map under Homo can have collisions.
    // reverse maps target→pattern_idx. If 0→0 AND 2→0, reverse[0]=0 (first) then 2 (overwritten).
    // Wait, that's a HashMap. The last write wins. So reverse may be incomplete.
    // Ban's injectivity check: self.reverse.contains_key(&n). Under Homo, reverse
    // is updated but can be stale. Does this cause incorrect ban filtering?
    //
    // Actually: under Homo, the ban_backtrack skips the reverse check entirely
    // (Homo branch does nothing). So ban(Mono) still checks reverse, but the
    // reverse map was built by the Homo get, which may have overwrites.

    println!("--- Homo get with collapse + Mono ban: reverse map integrity ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(verbose &g, search![
        get(Homo) {
            N(0) ^ (N(1) ^ N(2))
        },
        ban(Mono) {
            n(0) ^ N(3)
                 ^ n(2)
        }
    ]);

    // Q: The reverse map issue in detail:
    // mapping = [Some(N(0)), Some(N(1)), Some(N(0))]  (indices 0,1,2)
    // reverse = {N(0) → 2, N(1) → 1}  (N(0) was inserted for idx 0, then overwritten for idx 2)
    // Ban(Mono) checks: self.reverse.contains_key(&n) for candidate n.
    // So N(0) and N(1) are both "taken". Ban_only N(3) can only use... nothing else exists.
    // But N(0) is mapped to BOTH pattern nodes 0 and 2 under Homo!
    // The reverse map only records the LAST one. Is this a problem?
    //
    // For ban(Mono): the check prevents ban_only from colliding with ANY get node.
    // Since reverse has N(0)→2 and N(1)→1, ban(Mono) correctly blocks both targets.
    // But it lost the info that pattern node 0 also maps to N(0).
    // For the ban check this doesn't matter — the candidate is blocked regardless.
    //
    // For ban(Homo): the check is skipped entirely. N(3) can reuse any target.
    // So ban(Homo) has different semantics than ban(Mono) even when get is Homo.

    Ok(())
}
