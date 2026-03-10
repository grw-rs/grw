use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- ban % on Anydir catches cross-slot ---");

    let g: graph::Anydir0 = graph![
        N(0) >> N(1)
             ^ N(2)
    ]?;

    trace!(&g, search![
        get(Mono) {
            N(0) >> N(1)
        },
        ban(Mono) {
            n(0) % N(2)
        }
    ]);

    Ok(())
}
