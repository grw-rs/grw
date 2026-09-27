use grw::composite::{Composite, Part, Typed};
use grw::TypeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Signs {
    since: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Uses {
    service: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Other;

#[derive(Debug, Clone, PartialEq, Eq, Part)]
enum Rel {
    Signs(Signs),
    Uses(Uses),
}

fn main() {
    let set = TypeSet::from_part(Rel::Signs(Signs { since: 1 }));
    let _ = set.get::<Other>();
}
