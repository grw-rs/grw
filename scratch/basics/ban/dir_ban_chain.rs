use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- dir: ban chain ---");

    let g: graph::Dir0 = graph![
        N_() >> N_()
    ]?;

    trace!(verbose &g, search![
        get(Mono) {
            N(0) >> N(1) >> N(2)
        },
        ban(Mono) {
            n(0) >> N_()
        }
    ]);

    Ok(())
}
