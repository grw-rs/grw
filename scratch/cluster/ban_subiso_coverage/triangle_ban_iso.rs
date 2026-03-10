use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: What about ban(Iso)? Should ban_only nodes require
    //    exact degree match against the ban pattern?
    //    Currently: no. ban(Iso) only means "ban_only nodes must be injective
    //    w.r.t. get mapping and each other" — same as Mono.

    println!("--- ban(Iso) vs ban(Mono): same behavior? ---");

    println!("  ban(Iso):");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Iso) {
            n(1) ^ N(3)
        }
    ]);

    Ok(())
}
