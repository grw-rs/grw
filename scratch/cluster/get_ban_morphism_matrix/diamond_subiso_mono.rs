use grw::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- diamond: get(SubIso) + ban(Mono) ---");

    let g: graph::Undir0 = graph![
        N(0) ^ N(1)
             ^ N(2)
             ^ N(3),
        n(1) ^ n(2)
             ^ n(3)
    ]?;

    
    // trace!(verbose &g, search![
    //     get(SubIso) {
    //         N(10) ^ N(11)
    //     },
    //     ban(Mono) {
    //         n(10) ^ (N(12) ^ n(11))
    //     }
    // ]);

    trace!(verbose &g, search![
        get(SubIso) {
            N(10) ^ N(11).val(10)
        },
        ban(Mono) {
            n(10) ^ (N(12) ^ n(11))
        }
    ]);

    
    Ok(())
}



