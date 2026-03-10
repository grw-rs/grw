use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- test(true) trivially matches ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(&g, search![
        get(Mono) {
            N(0) & E().test(|_: &()| true) % N(1)
        }
    ]);

    Ok(())
}
