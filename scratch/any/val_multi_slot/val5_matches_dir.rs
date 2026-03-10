use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let g: graph::AnydirE<i32> = graph![
        N(0) & E().val(5) >> N(1),
        n(0) & E().val(3) ^ n(1)
    ]?;

    println!("--- val(5): matches dir slot ---");

    trace!(&g, search![
        get(Mono) {
            N(0) & E().val(5) % N(1)
        }
    ]);

    Ok(())
}
