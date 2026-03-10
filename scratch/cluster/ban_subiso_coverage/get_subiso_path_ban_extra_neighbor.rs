use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // On a clean path: SubIso matches, then ban checks for extra neighbor of middle node

    println!("--- get(SubIso) path + ban extra neighbor ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ N(3)
    ]?;

    trace!(verbose &g, search![
        get(SubIso) {
            N(10) ^ (N(11) ^ N(12))
        },
        ban(Mono) {
            n(10) ^ N(20)
        }
    ]);

    Ok(())
}
