use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- X(0) on Anydir: pins to node 0 (dir+undir) ---");

    let g: graph::Anydir0 = graph![
        N(0) >> (N(1) ^ N(2))
             << n(1)
    ]?;

    trace!(&g,
        search![
            get(SubIso) {
                X(0) % N(1)
            }
        ],
        &[(0, 0)]
    );

    Ok(())
}
