use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Homo ban: does mapping ban_only to self change anything? ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(verbose &g, search![
        get(Homo) {
            N(0) ^ N(1)
        },
        ban(Homo) {
            n(0) ^ (N(2) ^ n(1))
        }
    ]);

    Ok(())
}
