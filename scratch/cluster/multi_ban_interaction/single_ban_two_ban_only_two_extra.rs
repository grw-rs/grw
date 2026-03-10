use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- single ban, 2 ban_only, target has 2 extra neighbors ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2)
                     ^ N(3))
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(1) ^ N(5)
                 ^ N(6)
        }
    ]);

    Ok(())
}
