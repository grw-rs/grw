use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- SubIso % covers single Undir edge ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
    ]?;

    trace!(&g, search![
        get(SubIso) {
            N(0) % N(1)
        }
    ]);

    Ok(())
}
