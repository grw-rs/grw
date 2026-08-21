use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let g: graph::AnydirE<i32> = graph![
        N(0) & E().val(5) >> N(1),
        n(0) & E().val(3) ^ n(1)
    ]?;

    println!("--- test(>4): matches dir slot (5>4) ---");

    trace!(&g, search![
        get(Mono) {
            N(0) & E().test(|v| *v > 4) % N(1)
        }
    ]);

    Ok(())
}
