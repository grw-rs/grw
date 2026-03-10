use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ban has ban_only_node N(2). Under Mono it can't reuse a get-mapped target.
    // Under Homo it can — so Homo ban is easier to satisfy (= stricter filter).
    //
    // Q: Does ban(Homo) reject MORE matches than ban(Mono) when the ban_only_node
    //    could map to a get-mapped node?

    // target: triangle 0-1-2
    // get: edge 0-1 (Mono) → 6 matches
    // ban: n(0) ^ N(2) ^ n(1)  i.e. "a common neighbor exists"
    //
    // Under ban(Mono): N(2) can't reuse 0 or 1. If get maps {0→a, 1→b},
    //   ban needs c ∉ {a,b} adjacent to both. In a triangle, the 3rd vertex works.
    //   → all 6 matches banned
    //
    // Under ban(Homo): N(2) CAN reuse a or b.
    //   Even if no 3rd vertex exists, N(2) could map to a (already mapped to n(0)).
    //   That means: ban needs "some node adjacent to both a and b".
    //   In a triangle, a is adjacent to b (so N(2)→a works).
    //   → also all 6 banned... but for different reasons

    println!("--- triangle: ban(Mono) common neighbor ---");

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
            n(1) ^ n(2)
        }
    ]);

    Ok(())
}
