use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- X(0) & E().val(5): pins + val filter ---");

    let g: graph::UndirE<i32> = graph![
        N(0) & E().val(5) ^ N(1),
        n(0) & E().val(3) ^ N(2)
    ]?;

    trace!(&g,
        search![
            get(Mono) {
                X(0) & E().val(5) % N(1)
            }
        ],
        &[(0, 0)]
    );

    Ok(())
}
