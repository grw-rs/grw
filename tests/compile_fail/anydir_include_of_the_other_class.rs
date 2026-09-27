use grw::composite::{Part, TypeSet};
use grw::graph::edge::Anydir;
use grw::graph::MGraph;

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
    let mut g: MGraph<(), Anydir<TypeSet<Tie>, TypeSet<Rel>>> = grw::mgraph![N(0), N(1)].unwrap();
    let _ = g.modify(grw::modify![x(0) & E().include(Signs) ^ x(1)]);
}
