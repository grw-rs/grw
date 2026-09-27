use grw::composite::Part;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Signs {
    since: u32,
}

#[derive(Part)]
enum Rel {
    Signs(Signs),
    Empty,
}

fn main() {}
