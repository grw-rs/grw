use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: Does "ban(M) { n(0) ^ !N_() }" mean:
    //   (a) "reject if node 0's match has a neighbor" (negation applied at ban level)
    //   (b) "reject if ∃ node not-adjacent to 0's match" (ban ∧ negation)
    //   (c) something else entirely?
    //
    // Need to trace through the runtime to understand actual semantics.

    // Multiple negated nodes in ban:

    println!("--- ban with two negated nodes ---");

    let g: graph::Undir0 = graph![
        N(0) ^ (N(1) ^ N(2))
             ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) ^ N(1)
        },
        ban(Mono) {
            n(0) ^ !N(2),
            n(1) ^ !N(3)
        }
    ]);

    Ok(())
}
