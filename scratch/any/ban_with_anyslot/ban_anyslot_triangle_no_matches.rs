use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- ban % on triangle: every node has extra neighbor, 0 matches ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ (N(2) ^ n(1))
    ]?;

    trace!(&g, search![
        get(Mono) {
            N(0) % N(1)
        },
        ban(Mono) {
            n(0) % N(2)
        }
    ]);

    Ok(())
}
