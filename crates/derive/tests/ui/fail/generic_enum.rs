use grw::composite::Part;

#[derive(Debug, Clone, PartialEq, Part)]
enum Rel<T> {
    Signs(T),
    Uses(u32),
}

fn main() {}
