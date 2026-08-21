use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compare: all in one SubIso cluster

    println!("--- same pattern, single SubIso cluster ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ N(2),
        n(1) ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(SubIso) {
            N(10) ^ N(11)
                  ^ N(12)
        }
    ]);

    Ok(())
}
