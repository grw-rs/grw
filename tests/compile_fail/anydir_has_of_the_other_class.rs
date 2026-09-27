use grw::composite::{Part, TypeSet};
use grw::graph::edge::Anydir;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Knows;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Signs;

#[derive(Debug, Clone, PartialEq, Eq, Part)]
enum Tie {
    Knows(Knows),
}

#[derive(Debug, Clone, PartialEq, Eq, Part)]
enum Rel {
    Signs(Signs),
}

fn main() {
    let _ = grw::search![<(), Anydir<TypeSet<Tie>, TypeSet<Rel>>>; get(Mono) { N(0) & E().has::<Signs>() ^ N(1) }];
}
