use grw::graph::{edge, MGraph};
use grw::{id, mgraph};

fn main() {
    let mut g: MGraph<u32, edge::Undir<()>> = mgraph![N(0).val(1u32) ^ N(1).val(2u32)].unwrap();
    // Node values feed the index tables; they change only through `modify!`.
    *g.get_mut(id::N(0)).unwrap() = 777u32;
}
