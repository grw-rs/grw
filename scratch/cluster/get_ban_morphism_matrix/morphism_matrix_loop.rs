use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Systematic test: all 4x4 get×ban morphism combinations
    // on a diamond (K4 minus one edge): 0-1, 0-2, 0-3, 1-2, 1-3 (missing 2-3)
    // get: edge. ban: common neighbor with ban_only node.

    let target: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ N(2)
             ^ N(3),
        n(1) ^ n(2)
             ^ n(3)
    ]?;

    for &get_m in &[Iso, SubIso, Mono, Homo] {
        for &ban_m in &[Iso, SubIso, Mono, Homo] {
            let label = format!("get({get_m:?})+ban({ban_m:?})");
            let pattern = search![
                get(get_m) {
                    N(10) ^ N(11)
                },
                ban(ban_m) {
                    n(10) ^ (N(12) ^ n(11))
                }
            ];
            match compile::<(), edge::Undir<()>>(pattern) {
                Ok(grw::search::Search::Resolved(r)) => {
                    let query = r.into_query();
                    let indexed = target.index(RevCsr);
                    let count = Seq::search(&query, &indexed).count();
                    println!("  {label}: OK - valid ({count} matches)");
                }
                Ok(grw::search::Search::Unresolved(_)) => {
                    println!("  {label}: OK - valid (bound)");
                }
                Err(e) => {
                    println!("  {label}: OK - rejected ({e})");
                }
            }
        }
    }

    Ok(())
}
