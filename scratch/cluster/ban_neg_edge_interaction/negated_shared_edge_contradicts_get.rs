use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ban with negated edge between two shared (get-mapped) nodes.
    // ban { n(0) & !E() ^ n(1) }
    // "ban if matched[0] NOT adjacent to matched[1]"
    // But get already requires them adjacent... contradiction?
    // This is a shared-edge check. The edge is negated + not ban_only.
    // ban_shared_edges_satisfied: checks if negated edge IS present → returns false (ban not sat).
    // Since get guarantees the edge exists, the negated shared edge always fails → ban never fires.
    // Effectively a no-op ban.

    println!("--- ban negated shared edge: contradicts get (no-op) ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) & !E() ^ n(1)
        }
    ]);

    Ok(())
}
