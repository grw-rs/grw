use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Both node AND edge pred in ban.
    // ban { n(0) & E().test(e_pred) ^ N(3).test(n_pred) }
    // "reject if ∃ neighbor c of matched[0] where edge has e_pred AND c has n_pred"

    println!("--- ban with both node and edge pred ---");

    let g: Graph<i32, edge::Undir<i32>> = graph![
        N(0).val(1) & E().val(5) ^ N(1).val(42),
        n(0) & E().val(3) ^ N(2).val(99)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) & E().test(|v| *v > 4) ^ N(3).val(42)
        }
    ]);
    // Ban fires if ∃ c: edge(matched[0],c).val > 4 AND c.val == 42.
    // For match {0→0,1→1}: edges from 0: (0,1,val=5), (0,2,val=3).
    //   c=1: val=5>4 AND val=42 → ban fires → rejected.
    //   But N(3) can't be 0 or 1 (Mono). c=2: val=3≤4, skip.
    //   Wait, can N(3) be 1? N(3) is ban_only, 1 is get-mapped.
    //   Under Mono: no. Under Homo: yes.
    //   So ban(Mono) can't use N(3)→1. N(3)→2: edge val=3, not>4. Ban not satisfied. Survives!

    Ok(())
}
