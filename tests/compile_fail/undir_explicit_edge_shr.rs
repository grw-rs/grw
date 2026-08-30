use grw::modify::*;

fn main() {
    let mut g = grw::graph::MUndir0::default();
    let _ = g.modify(grw::modify![
        N(1) & E() >> N(2),
    ]);
}
