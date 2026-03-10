use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Q: SubIso in one cluster, Mono in another, sharing a node.
    //    The SubIso coverage check looks at ALL pattern edges for that node.
    //    A Mono cluster's edges also contribute to the "covered" set.
    //    Is a match where the Mono-cluster edge exists but there's also
    //    an uncovered target edge rejected (because SubIso)?

    println!("--- SubIso+Mono sharing node: coverage scope ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ N(2),
        n(1) ^ n(2)
    ]?;

    trace!(verbose &g, search![
        get(SubIso) {
            N(10) ^ N(11)
        },
        get(Mono) {
            n(10) ^ N(12)
        }
    ]);

    Ok(())
}
