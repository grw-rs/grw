use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- X(99) on graph without node 99: 0 matches ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(&g,
        search![
            get(Mono) {
                X(99) % N(1)
            }
        ],
        &[(99, 0)]
    );

    Ok(())
}
