use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- ban(Homo) with both node and edge pred ---");

    let g: Graph<i32, edge::Undir<i32>> = graph![
        N(0).val(1) & E().val(5) ^ N(1).val(42),
        n(0) & E().val(3) ^ N(2).val(99)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Homo) {
            n(0) & E().test(|v| *v > 4) ^ N(3).val(42)
        }
    ]);

    Ok(())
}
