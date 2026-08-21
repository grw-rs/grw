use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // "reject if ∃ common neighbor of ctx and matched[1]"

    println!("--- ban_only bridging context and free: triangle ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2)
    ]?;

    trace!(verbose &g,
        search![
            get(Mono) {
                X(0) ^ N(1)
            },
            ban(Mono) {
                x(0) ^ N(3),
                n(1) ^ n(3)
            }
        ],
        &[(0u32, 0u32)]
    );

    Ok(())
}
