use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Dir: ban negated reverse-slot, but bidirectional target ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1),
        n(1) >> n(0)
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> N(1)
        },
        ban(Mono) {
            n(0) & !E() << n(1)
        }
    ]);

    Ok(())
}
