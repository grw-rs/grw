use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- ban: ctx not-adjacent to ban_only (ctx 0→1, degree 2) ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
    ]?;

    trace!(verbose &g,
        search![
            get(Mono) {
                X(0) ^ N(1)
            },
            ban(Mono) {
                x(0) & !E() ^ N(3)
            }
        ],
        &[(0u32, 1u32)]
    );

    Ok(())
}
