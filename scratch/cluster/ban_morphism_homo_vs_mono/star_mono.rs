use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Star: center 0, leaves 1,2,3. Get edge, ban common neighbor.
    // Here ALL 3 leaves share center 0 as neighbor.
    // ban(Mono) N(2)→center works for matches not involving center.
    // ban(Homo) N(2) could also map to an already-matched leaf.

    println!("--- star: ban(Mono) common neighbor ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ N(2)
             ^ N(3)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(10) ^ N(11)
        },
        ban(Mono) {
            n(10) ^ N(12),
            n(11) ^ n(12)
        }
    ]);

    Ok(())
}
