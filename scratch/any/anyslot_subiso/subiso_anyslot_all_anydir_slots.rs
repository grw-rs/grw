use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- SubIso % covers all 3 Anydir slots ---");

    let g: graph::Anydir0 = graph![
        N(0) >> N(1)
             << n(1)
              ^ n(1)
    ]?;

    trace!(&g, search![
        get(SubIso) {
            N(0) % N(1)
        }
    ]);

    Ok(())
}
