use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let g: graph::AnydirE<i32> = graph![
        N(0) & E().val(5) >> N(1),
        n(0) & E().val(3) ^ n(1)
    ]?;

    println!("--- test(>10): neither slot passes ---");

    trace!(&g, search![
        get(Mono) {
            N(0) & E().test(|v| *v > 10) % N(1)
        }
    ]);

    Ok(())
}
