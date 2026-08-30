use grw::search::{Search, Seq, RevCsr};
use grw::Graph as _;
type ER = grw::edge::Undir<()>;

fn count(p: grw::Search<(), ER>, g: &grw::MGraph<(), ER>) -> usize {
    let t = g.index(RevCsr);
    let Search::Resolved(r) = p else { panic!() };
    Seq::search(&r.query(), &t).count()
}

#[test]
fn disconnected_bisect() {
    let g = grw::mgraph![<(), ER>; N(0) ^ N(1)].unwrap();

    let x1 = grw::search![<(), ER>; get(Morphism::Mono) { N(0) }];
    println!("x1 mono single: {}", count(x1.unwrap(), &g));

    let x2 = grw::search![<(), ER>; get(Morphism::Homo) { N(1) ^ N(2) }];
    println!("x2 homo pair alone: {}", count(x2.unwrap(), &g));

    let x3 = grw::search![<(), ER>;
        get(Morphism::Mono) { N(0) },
        get(Morphism::Mono) { N(1) }
    ];
    println!("x3 two mono disconnected: {}", count(x3.unwrap(), &g));

    let x4 = grw::search![<(), ER>;
        get(Morphism::Mono) { N(0) },
        get(Morphism::Homo) { N(1) }
    ];
    println!("x4 mono + homo disconnected: {}", count(x4.unwrap(), &g));

    let x5 = grw::search![<(), ER>;
        get(Morphism::Homo) { N(1) ^ N(2) },
        get(Morphism::Mono) { N(0) }
    ];
    println!("x5 probe5 clusters swapped: {}", count(x5.unwrap(), &g));
}

#[test]
fn disconnected_depth2() {
    // path a(0)-b(1)-c(2)
    let g = grw::mgraph![<(), ER>; N(0) ^ N(1), n(1) ^ N(2)].unwrap();

    let x6 = grw::search![<(), ER>;
        get(Morphism::Mono) { N(0) ^ N(1) },
        get(Morphism::Mono) { N(2) }
    ];
    println!("x6 mono edge + mono third (expect 4): {}", count(x6.unwrap(), &g));

    let x7 = grw::search![<(), ER>;
        get(Morphism::Mono) { N(0) ^ N(1) },
        get(Morphism::Homo) { N(2) }
    ];
    println!("x7 mono edge + homo third (expect 12): {}", count(x7.unwrap(), &g));

    let x8 = grw::search![<(), ER>;
        get(Morphism::Homo) { N(0) ^ N(1) },
        get(Morphism::Mono) { N(2) }
    ];
    println!("x8 homo edge + mono third (expect 12): {}", count(x8.unwrap(), &g));
}
