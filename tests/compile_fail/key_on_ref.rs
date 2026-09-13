use grw::graph::index::IndexName;
use grw::graph::{edge, MGraph};
use grw::{mgraph, search};

const BY_VAL: IndexName = IndexName("by_val");

fn main() {
    let g: MGraph<u32, edge::Undir<()>> = mgraph![N(0).val(1u32) ^ N(1).val(2u32)].unwrap();
    let _ = search![&g, get(Mono) { N(a) ^ n(a: key(BY_VAL, 1u32)) }];
}
