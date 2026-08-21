#![allow(unused_imports)]
use grw::*;
use grw::search::{engine::Match, error};

type UER = grw::graph::edge::Undir<()>;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DIVERGENCE 1: all_negated_nonempty ===");
    println!("Graph: N(0) ^ N(1)");
    let g: graph::Undir0 = graph![N(0) ^ N(1)]?;

    println!("\n--- Pattern A: get(Mono) {{ !N_() }} ---");
    trace!(verbose &g, search![<(), UER>;
        get(Mono) { !N_() }
    ]);

    println!("\n--- Pattern B: ban(Mono) {{ N_() }} ---");
    trace!(verbose &g, search![<(), UER>;
        ban(Mono) { N_() }
    ]);

    println!("\n=== DIVERGENCE 2: all_negated_test_has_match ===");
    println!("Graph: N(0).val(1) ^ N(1).val(200)");
    let g2: graph::Undir<i32, ()> = graph![N(0).val(1) ^ N(1).val(200)]?;

    println!("\n--- Pattern A: get(Mono) {{ !N_().test(|v| *v > 100) }} ---");
    trace!(verbose &g2, search![<i32, UER>;
        get(Mono) { !N_().test(|v: &i32| *v > 100) }
    ]);

    println!("\n--- Pattern B: ban(Mono) {{ N_().test(|v| *v > 100) }} ---");
    trace!(verbose &g2, search![<i32, UER>;
        ban(Mono) { N_().test(|v: &i32| *v > 100) }
    ]);

    println!("\n=== DIVERGENCE 3: all_negated_homo_nonempty ===");
    println!("Graph: N(0) ^ N(1)");

    println!("\n--- Pattern A: get(Homo) {{ !N_() }} ---");
    trace!(verbose &g, search![<(), UER>;
        get(Homo) { !N_() }
    ]);

    println!("\n--- Pattern B: ban(Homo) {{ N_() }} ---");
    trace!(verbose &g, search![<(), UER>;
        ban(Homo) { N_() }
    ]);

    println!("\n=== DIVERGENCE 4: negated_freestanding_no_isolated ===");
    println!("Graph: N(0) ^ N(1)  (only 2 nodes, no 3rd)");

    println!("\n--- Pattern A: get(Mono) {{ N(0) ^ N(1), !N_() }} ---");
    trace!(verbose &g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1), !N_() }
    ]);

    println!("\n--- Pattern B: get(Mono) {{ N(0) ^ N(1) }}, ban(Mono) {{ N_() }} ---");
    trace!(verbose &g, search![<(), UER>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { N_() }
    ]);

    Ok(())
}
