use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Same pattern in single cluster:

    println!("--- overlapping: same pattern, single Iso cluster ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ N(2),
        n(1) ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(Iso) {
            N(10) ^ N(11)
                  ^ N(12)
        }
    ]);

    Ok(())
}
