use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- test(false) on Dir rejects ---");

    let g: graph::Dir0 = graph![
        N(0) >> N(1)
    ]?;

    trace!(&g, search![
        get(Mono) {
            N(0) & E().test(|_: &()| false) % N(1)
        }
    ]);

    Ok(())
}
