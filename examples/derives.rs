use grw::composite::{Kinded, Part};
use grw::layout::Val;

#[derive(Debug, Clone, Copy, PartialEq, Val)]
struct Weight {
    w: f64,
    active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Signs {
    since: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Uses {
    service: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Part)]
enum Rel {
    Signs(Signs),
    Uses(Uses),
}

fn main() {
    let weight = Weight { w: 1.5, active: true };
    println!("{:?} has {} fields", weight, Weight::fields().len());
    let signs = Rel::from(Signs { since: 7 });
    let uses = Rel::from(Uses { service: "ledger".to_string() });
    println!("{:?} is {:?}, {:?} is {:?}", signs, signs.kind(), uses, uses.kind());
}
