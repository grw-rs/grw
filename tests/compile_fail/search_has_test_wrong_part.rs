use grw::composite::{Part, TypeSet};
use grw::graph::edge::Dir;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Signs {
    since: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Uses {
    service: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Part)]
enum Rel {
    Signs(Signs),
    Uses(Uses),
}

fn main() {
    let _ = grw::search![<(), Dir<TypeSet<Rel>>>; get(Mono) {
        N(0) & E().has::<Signs>().test(|u: &Uses| u.service > 1) >> N(1)
    }];
}
