use grw::graph::{edge, MGraph};
use grw::search::Pattern;
use grw::{mgraph, pattern, search};
fn main() {
    let g: MGraph<u8, edge::Undir<()>> = mgraph![N(0).val(1u8) ^ N(1).val(2u8)].unwrap();
    let p: Pattern<u8, edge::Undir<()>> = pattern![get(Mono) { N(a) ^ N(b) }].unwrap();
    let z = grw::id::N(0);
    let _ = search![&g, p with X(a = z), X(a = z)];
}
