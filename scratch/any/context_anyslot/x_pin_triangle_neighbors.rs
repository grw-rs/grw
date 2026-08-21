use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- X(0) pins to node 0: triangle, 2 neighbors ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ (N(2) ^ n(1))
    ]?;

    trace!(&g,
        search![
            get(Mono) {
                X(0) % N(1)
            }
        ],
        &[(0, 2)]
    );

    Ok(())
}
