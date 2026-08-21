use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- ctx 0→1: node 1 has neighbor 0 only ---");

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
        &[(0u32, 1u32)]
    );

    Ok(())
}
