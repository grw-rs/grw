use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Node pred in ban on ban_only node.
    // ban { n(0) ^ N(3).val(42) }
    // "reject if matched[0] has a neighbor with value 42"

    println!("--- ban with node pred on ban_only ---");

    let g: Graph<i32, edge::Undir<i32>> = graph![
        N(0).val(1) & E().val(0) ^ N(1).val(42),
        n(0) & E().val(0) ^ N(2).val(99)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) ^ N(3).val(42)
        }
    ]);

    Ok(())
}
