use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("  runtime on 0→1→2, 1→3 (extra outgoing from 1):");

    let g: graph::Dir0 = graph![
        N(0) >> (N(1) >> N(2)),
        n(1) >> N(3)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> (N(1) >> N(2))
        },
        ban(Mono) {
            n(1) >> N_()
        }
    ]);

    Ok(())
}
