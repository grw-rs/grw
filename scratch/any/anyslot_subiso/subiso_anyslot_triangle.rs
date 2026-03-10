use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- SubIso triangle via % ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ (N(2) ^ n(1))
    ]?;

    trace!(&g, search![
        get(SubIso) {
            N(0) % N(1)
                 % (N(2) % n(1))
        }
    ]);

    Ok(())
}
