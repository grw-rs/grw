use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Anydir dir+undir: % + !>> + !<< (dir violates) ---");

    let g: graph::Anydir0 = graph![
        N(0) >> N(1)
             ^ n(1)
    ]?;

    trace!(&g, search![
        get(Mono) {
            N(0) % N(1),
            n(0) & !E() >> n(1),
            n(0) & !E() << n(1)
        }
    ]);

    Ok(())
}
