use grw::graph::{edge, MGraph};
use grw::{mgraph, search};
fn main() {
    let g: MGraph<u8, edge::Undir<()>> = mgraph![N(0).val(1u8) ^ N(1).val(2u8)].unwrap();
    let _ = search![&g get(Mono) { N(a) ^ N(b) }];
}
