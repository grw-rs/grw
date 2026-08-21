use grw::*;

fn expect_reject<T, E: std::fmt::Display>(label: &str, result: Result<T, E>) {
    match result {
        Err(e) => println!("  {label}: OK - rejected ({e})"),
        Ok(_) => println!("  {label}: FAIL - expected rejection, compiled"),
    }
}

fn run<ER: graph::Edge>(
    label: &str,
    result: Result<Search<(), ER>, search::error::Search>,
    target: &graph::Graph<(), ER>,
    expected: usize,
) {
    let Search::Resolved(r) = result.unwrap()
    else { panic!("unexpected bound") };
    let query = r.into_query();
    let n = Seq::search(&query, target).collect::<Vec<_>>().len();
    let status = if n == expected { "OK" } else { "FAIL" };
    println!("  {label}: {status} - {n} match(es) (expect {expected})");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {

    // =========================================================================
    // § Compile-Time Rejection: Same edge_map key (doc §60-64)
    // Same slot between shared nodes → always ConflictingPred or RedundantInBan
    // =========================================================================

    println!("--- compile: same-slot overlap ---");

    expect_reject("ban pred on same-slot (undir)", search![<i32, edge::Undir<i32>>;
        get(Mono) { N(0) ^ (N(1) ^ N(2)) },
        ban(Mono) { n(0) & E().test(|v: &i32| *v > 10) ^ n(1) }
    ]);

    expect_reject("ban val on same-slot (undir)", search![<i32, edge::Undir<i32>>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(0) & E().val(42) ^ n(1) }
    ]);

    // =========================================================================
    // § Compile-Time Rejection: Ban % when all slots covered (doc §66-73)
    // =========================================================================

    println!("\n--- compile: ban % all slots covered ---");

    expect_reject("undir: ^ covers only slot → % dead", search![<(), edge::Undir<()>>;
        get(Mono) { N(0) ^ N(1) },
        ban(Mono) { n(0) % n(1) }
    ]);

    expect_reject("dir: >> + << covers both slots → % dead", search![<(), edge::Dir<()>>;
        get(Mono) { N(0) >> N(1) },
        get(Mono) { n(0) << n(1) },
        ban(Mono) { n(0) % n(1) }
    ]);

    // =========================================================================
    // § Compile-Time Rejection: SubIso/Iso get (doc §75-77)
    // Any ban edge between shared nodes is dead under SubIso/Iso get
    // =========================================================================

    println!("\n--- compile: SubIso/Iso get kills all ban shared edges ---");

    expect_reject("dir: SubIso get >>, ban <<", search![<(), edge::Dir<()>>;
        get(SubIso) { N(0) >> N(1) },
        ban(Mono) { n(0) << n(1) }
    ]);

    expect_reject("dir: SubIso get >>, ban %", search![<(), edge::Dir<()>>;
        get(SubIso) { N(0) >> N(1) },
        ban(Mono) { n(0) % n(1) }
    ]);

    // =========================================================================
    // § Runtime: Uncovered slot logic — Dir (doc §121-124)
    // get >> on Dir: ban % only checks << slot
    // =========================================================================

    println!("\n--- runtime: dir, ban % checks uncovered << slot ---");

    let g_dir1: graph::Dir0 = graph![N(0) >> N(1)]?;
    run("no reverse edge → survives", search![
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(0) % n(1) }
    ], &g_dir1, 1);

    let g_dir2: graph::Dir0 = graph![N(0) >> (N(1) >> n(0))]?;
    run("reverse edge exists → rejected", search![
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(0) % n(1) }
    ], &g_dir2, 0);

    // =========================================================================
    // § Runtime: Uncovered slot logic — Anydir (doc §114-119)
    // get >> on Anydir: ban % checks << and ^ slots
    // =========================================================================

    println!("\n--- runtime: anydir, ban % checks uncovered << and ^ slots ---");

    let g_any1: graph::Anydir0 = graph![N(0) >> N(1)]?;
    run("only >> edge → ban % finds nothing uncovered → survives", search![
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(0) % n(1) }
    ], &g_any1, 1);

    let g_any2: graph::Anydir0 = graph![N(0) >> N(1), n(0) ^ n(1)]?;
    run(">> + ^ edges → ban % finds ^ uncovered → rejected", search![
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(0) % n(1) }
    ], &g_any2, 0);

    let g_any3: graph::Anydir0 = graph![N(0) >> N(1), n(1) >> n(0)]?;
    run(">> + << edges → ban % finds << uncovered → rejected", search![
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(0) % n(1) }
    ], &g_any3, 0);

    // =========================================================================
    // § Runtime: Ban specific uncovered slot — Anydir (doc §117-118)
    // get >> on Anydir: ban ^ checks only undir slot
    // =========================================================================

    println!("\n--- runtime: anydir, ban specific uncovered slot ---");

    let g_any4: graph::Anydir0 = graph![N(0) >> N(1), n(0) ^ n(1)]?;
    run("ban ^ on anydir: ^ exists → rejected", search![
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(0) ^ n(1) }
    ], &g_any4, 0);

    let g_any5: graph::Anydir0 = graph![N(0) >> N(1), n(1) >> n(0)]?;
    run("ban ^ on anydir: only << exists, no ^ → both dirs survive", search![
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(0) ^ n(1) }
    ], &g_any5, 2);

    run("ban << on anydir: << exists → rejected", search![
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(0) << n(1) }
    ], &g_any5, 0);

    // =========================================================================
    // § Runtime: Disconnected get nodes + ban % (doc §131-134)
    // Zero covered slots → ban % checks all slots → non-adjacent pairs only
    // =========================================================================

    println!("\n--- runtime: disconnected get nodes, ban % rejects adjacent ---");

    let g_disc: graph::Anydir0 = graph![N(0) >> N(1), N(2)]?;
    run("3 nodes, 0→1 edge: 6 pairs minus 2 adjacent = 4", search![
        get(Mono) { N(0), N(1) },
        ban(Mono) { n(0) % n(1) }
    ], &g_disc, 4);

    let g_disc2: graph::Anydir0 = graph![N(0), N(1), N(2)]?;
    run("3 isolated nodes: no edges, all 6 pairs survive", search![
        get(Mono) { N(0), N(1) },
        ban(Mono) { n(0) % n(1) }
    ], &g_disc2, 6);

    // =========================================================================
    // § Runtime: Ban morphism — SubIso/Iso (doc §79-102)
    // ban(SubIso) fires only when induced subgraph exactly matches pattern
    // =========================================================================

    println!("\n--- runtime: ban(SubIso) morphism enforcement ---");

    let g_bsub1: graph::Anydir0 = graph![N(0) >> (N(1) >> n(0)), N(2), N(3)]?;
    run("bidirectional (0↔1): 2 edges > 1 ban edge → SubIso fails → all 12 survive", search![
        get(Mono) { N(0), N(1) },
        ban(SubIso) { n(0) % n(1) }
    ], &g_bsub1, 12);

    let g_bsub2: graph::Anydir0 = graph![N(0) >> N(1), N(2)]?;
    run("single edge (0→1): exactly 1 → SubIso matches → ban fires → 4 survive", search![
        get(Mono) { N(0), N(1) },
        ban(SubIso) { n(0) % n(1) }
    ], &g_bsub2, 4);

    let g_bsub3: graph::Anydir0 = graph![N(0), N(1), N(2)]?;
    run("isolated: 0 edges → ban edge not satisfied → all 6 survive", search![
        get(Mono) { N(0), N(1) },
        ban(SubIso) { n(0) % n(1) }
    ], &g_bsub3, 6);

    let g_bsub4: graph::Anydir0 = graph![N(0) >> (N(1) >> n(0)), n(0) >> N(2)]?;
    run("extra edge on unspecified pair → SubIso fails → all 3 survive", search![
        get(Mono) { N(0) >> N(1), N(2) },
        ban(SubIso) { n(0) % n(1), n(2) }
    ], &g_bsub4, 3);

    let g_bsub5: graph::Anydir0 = graph![N(0) >> (N(1) >> n(0)), N(2)]?;
    run("no extra edge on unspecified pair → SubIso matches → ban fires → 0", search![
        get(Mono) { N(0) >> N(1), N(2) },
        ban(SubIso) { n(0) % n(1), n(2) }
    ], &g_bsub5, 0);

    // =========================================================================
    // § Runtime: Multiple ban clusters — OR semantics (doc §47)
    // If ANY ban fires, the match is rejected
    // =========================================================================

    println!("\n--- runtime: multiple bans OR semantics ---");

    let g_multi: graph::Anydir0 = graph![N(0) >> N(1), n(0) ^ n(1), N(2)]?;
    run("two bans: ban << (no), ban ^ (yes) → rejected (OR)", search![
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(0) << n(1) },
        ban(Mono) { n(0) ^ n(1) }
    ], &g_multi, 0);

    let g_multi2: graph::Anydir0 = graph![N(0) >> N(1), N(2)]?;
    run("two bans: ban << (no), ban ^ (no) → both miss → survives", search![
        get(Mono) { N(0) >> N(1) },
        ban(Mono) { n(0) << n(1) },
        ban(Mono) { n(0) ^ n(1) }
    ], &g_multi2, 1);

    Ok(())
}
