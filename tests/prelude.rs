use grw::graph::edge::Dir;
use grw::prelude::*;
use grw::TypeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signs {
    pub since: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uses {
    pub service: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Part)]
pub enum Rel {
    Signs(Signs),
    Uses(Uses),
}

type Link = TypeSet<Rel>;

#[derive(Debug, Clone, PartialEq, Val)]
pub struct Account {
    pub id: u32,
}

#[test]
fn the_prelude_alone_writes_builds_changes_and_searches_a_graph() {
    let mut g = mgraph![<Account, Dir<Link>>;
        N(0).val(Account { id: 1 }) & E().include(Signs { since: 2023 }) >> N(1).val(Account { id: 2 }),
    ]
    .unwrap();
    modify!(g, [X(0) & E().include(Uses { service: 7 }) >> x(1)]).unwrap();

    let held = grw::Graph::get_edge_val(&g, grw::graph::edge::dir::E::D(0, 1)).unwrap();
    assert_eq!(held.get::<Uses>(), Some(&Uses { service: 7 }));
    assert_eq!(held.parts().count(), 2);

    let s = search![&g, get(Mono) { N(a) & E().has::<Signs>().has::<Uses>() >> N(b) }].unwrap();
    assert_eq!(s.iter().count(), 1);

    let stored = pattern![get(Mono) { N(a) & E().has::<Signs>() >> N(b) }].unwrap();
    assert_eq!(search![&g, stored].unwrap().iter().count(), 1);
}

#[test]
fn seek_is_in_scope_outside_the_macros() {
    use grw::search::dsl::{n, E, Op};
    let _op: Op<Account, Dir<Link>> = (n::<Account, Dir<Link>>(0) & E::<Account, Dir<Link>>().has::<Signs>() >> n(1)).into();
}
