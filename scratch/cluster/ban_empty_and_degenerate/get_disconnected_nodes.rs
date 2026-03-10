use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Case 6: get with NO edges (isolated nodes).

    println!("--- get with disconnected nodes ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1),
        N(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(10), N(11)
        }
    ]);

    Ok(())
}
