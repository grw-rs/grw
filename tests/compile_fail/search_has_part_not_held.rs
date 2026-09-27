use grw::composite::{Part, TypeSet};
use grw::graph::edge::Dir;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Signs;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Stranger;

#[derive(Debug, Clone, PartialEq, Eq, Part)]
enum Rel {
    Signs(Signs),
}

fn main() {
    let _ = grw::search![<(), Dir<TypeSet<Rel>>>; get(Mono) { N(0) & E().has::<Stranger>() >> N(1) }];
}
