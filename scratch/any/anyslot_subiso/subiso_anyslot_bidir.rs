use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- SubIso % covers both Dir slots (bidirectional) ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
             << n(1)
    ]?;

    trace!(&g, search![
        get(SubIso) {
            N(0) % N(1)
        }
    ]);

    Ok(())
}
