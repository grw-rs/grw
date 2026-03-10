use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Ban with >> between shared nodes, get has <<.
    // get: N(0)<<N(1) (= "1 points to 0"). ban: n(0)>>n(1) (= "0 points to 1").
    // Different slots! The ban checks a different edge than get.
    // If only 1→0 exists (no 0→1), ban's >> doesn't exist → ban not satisfied → match survives.
    // If 0→1 also exists, ban fires.

    println!("--- Dir: get << ban >> (different slots on shared) ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) << N(1)
        },
        ban(Mono) {
            n(0) >> n(1)
        }
    ]);

    Ok(())
}
