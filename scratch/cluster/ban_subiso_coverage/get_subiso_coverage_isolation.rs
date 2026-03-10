use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Edge case: get(SubIso) with ban.
    // The get match enforces SubIso coverage on get nodes.
    // Then ban checks are applied on top. Do ban edges affect SubIso coverage?
    //
    // Current: ban_only edges are skipped in is_feasible SubIso check (line 243).
    // So get(SubIso) coverage only considers get edges.
    // But a ban that REFERENCES shared edges (non-ban_only edges) —
    // those edges are already counted in get's coverage check.

    println!("--- get(SubIso) + ban: coverage isolation ---");

    // Path 0-1-2 with extra 0-2 edge (triangle)
    // get(SubIso) path 10-11-12: requires NO extra edges between matched nodes.
    // In triangle: 0-2 is extra for match {10→0,11→1,12→2}. SubIso rejects.
    // ban shouldn't affect this.

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(SubIso) {
            N(10) ^ (N(11) ^ N(12))
        },
        ban(Mono) {
            n(11) ^ N(20)
        }
    ]);

    Ok(())
}
