use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ (N(2) ^ n(1))
    ]?;

    println!("--- bare % vs val(()) give identical results on triangle ---");
    println!("  val(()) %:");

    trace!(&g, search![
        get(Mono) {
            N(0) & E().val(()) % N(1)
        }
    ]);

    Ok(())
}
