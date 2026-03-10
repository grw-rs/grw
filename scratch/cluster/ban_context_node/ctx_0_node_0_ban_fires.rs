use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // target: 0-1, 0-2
    // get: X(0)^N(1). ban: x(0)^N(2) — "reject if ctx has extra neighbor"

    println!("--- ctx 0→0: node 0 has neighbor 1 AND 2, ban fires ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ N(2)
    ]?;

    trace!(verbose &g,
        search![
            get(Mono) {
                X(0) ^ N(1)
            },
            ban(Mono) {
                x(0) ^ N(2)
            }
        ],
        &[(0u32, 0u32)]
    );

    Ok(())
}
