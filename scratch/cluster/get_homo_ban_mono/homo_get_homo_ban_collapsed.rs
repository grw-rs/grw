use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Homo get + Homo ban: ban_only can reuse collapsed target ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(verbose &g, search![
        get(Homo) {
            N(0) ^ (N(1) ^ N(2))
        },
        ban(Homo) {
            n(0) ^ N(3)
                 ^ n(2)
        }
    ]);

    Ok(())
}
