use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- multiple % with different preds pin uniquely ---");

    let g: graph::UndirE<i32> = graph![
        N(0) & E().val(5) ^ N(1),
        n(0) & E().val(3) ^ N(2),
        n(0) & E().val(7) ^ N(3)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) & E().val(5) % N(1)
                 & E().val(3) % N(2)
        }
    ]);

    Ok(())
}
