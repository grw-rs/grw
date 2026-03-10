use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- X(0) % X(1): both pinned, single match ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ n(1))
    ]?;

    trace!(&g,
        search![
            get(Homo) {
                X(0) % X(1)
            }
        ],
        &[(0, 0), (1, 1)]
    );

    Ok(())
}
