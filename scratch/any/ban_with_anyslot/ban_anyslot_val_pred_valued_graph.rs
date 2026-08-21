use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- ban % with val pred on valued graph ---");

    let g: graph::UndirE<i32> = graph![
        N(0) & E().val(5) ^ N(1),
        n(0) & E().val(3) ^ N(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) & E().test(|v| *v > 0) % N(1)
        },
        ban(Mono) {
            n(0) & E().val(3) % N(2)
        }
    ]);

    Ok(())
}
