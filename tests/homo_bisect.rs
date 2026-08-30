use grw::search::{Search, Seq, RevCsr};
use grw::Graph as _;
type ER = grw::edge::Undir<()>;

fn count(p: grw::Search<(), ER>, g: &grw::MGraph<(), ER>) -> usize {
    let t = g.index(RevCsr);
    let Search::Resolved(r) = p else { panic!() };
    Seq::search(&r.query(), &t).count()
}

#[test]
fn bisect() {
    // target: hub 0, leaves 1,2, pads 3,4,5
    let g = grw::mgraph![<(), ER>;
        N(0) ^ N(1), n(0) ^ N(2), N(3), N(4), N(5)
    ].unwrap();

    // d1: no mono leaf — homo 0,1 on hub + pendants
    let d1 = grw::search![<(), ER>;
        get(Morphism::Mono) { N(2) },
        get(Morphism::Homo) { N(0) ^ n(2), N(1) ^ n(2), N(4) ^ n(0), N(5) ^ n(1) }
    ];
    let c1 = count(d1.unwrap(), &g); println!("d1 (no mono leaf, 2 pendants): {c1}"); assert_eq!(c1, 12);

    // d2: mono leaf, ONE pendant
    let d2 = grw::search![<(), ER>;
        get(Morphism::Mono) { N(2) ^ N(3) },
        get(Morphism::Homo) { N(0) ^ n(2), N(1) ^ n(2), N(4) ^ n(0) }
    ];
    println!("d2 (mono leaf, 1 pendant): {}", count(d2.unwrap(), &g));

    // d3: no pendants, mono leaf (= probe1 shape on padded target)
    let d3 = grw::search![<(), ER>;
        get(Morphism::Mono) { N(2) ^ N(3) },
        get(Morphism::Homo) { N(0) ^ n(2), N(1) ^ n(2) }
    ];
    let c3 = count(d3.unwrap(), &g); println!("d3 (probe1 shape, padded target): {c3}"); assert_eq!(c3, 10);

    // d4: single homo pendant chain, no second homo on hub
    let d4 = grw::search![<(), ER>;
        get(Morphism::Mono) { N(2) },
        get(Morphism::Homo) { N(0) ^ n(2), N(4) ^ n(0) }
    ];
    println!("d4 (hub + homo leaf + homo pendant): {}", count(d4.unwrap(), &g));
}

// Same LOGICAL query as probe1 twice — only cluster declaration order of the
// mono leaf differs. Tie-breaks in the greedy search order follow declaration
// order, so variant B binds homo nodes before the mono leaf.
#[test]
fn declaration_order_changes_semantics() {
    let g = grw::mgraph![<(), ER>;
        N(0) ^ N(1), n(0) ^ N(2), n(0) ^ N(3)
    ].unwrap();

    let a = grw::search![<(), ER>;
        get(Morphism::Mono) { N(2) ^ N(3) },
        get(Morphism::Homo) { N(0) ^ n(2), N(1) ^ n(2) }
    ];
    let b = grw::search![<(), ER>;
        get(Morphism::Mono) { N(2) },
        get(Morphism::Homo) { N(0) ^ n(2), N(1) ^ n(2) },
        get(Morphism::Mono) { N(3) ^ n(2) }
    ];
    let t = g.index(RevCsr);
    let get_set = |p: grw::Search<(), ER>| -> std::collections::BTreeSet<[u32;4]> {
        let Search::Resolved(r) = p else { panic!() };
        Seq::search(&r.query(), &t)
            .map(|m| [ *m[0] as u32, *m[1] as u32, *m[2] as u32, *m[3] as u32 ])
            .collect()
    };
    let sa = get_set(a.unwrap());
    let sb = get_set(b.unwrap());
    println!("variant A (mono leaf first): {} matches", sa.len());
    println!("variant B (mono leaf last):  {} matches", sb.len());
    for m in sb.difference(&sa) { println!("  only-in-B: 0->{} 1->{} 2->{} 3->{}", m[0],m[1],m[2],m[3]); }
    for m in sa.difference(&sb) { println!("  only-in-A: 0->{} 1->{} 2->{} 3->{}", m[0],m[1],m[2],m[3]); }
    assert_eq!(sa, sb, "same logical query, different declaration order => different matches");
}
