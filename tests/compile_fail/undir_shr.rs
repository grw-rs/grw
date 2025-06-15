use grw::modify::*;

fn main() {
    let mut g = grw::graph::Undir0::default();
    let _ = g.modify(grw::modify![
        N(1) >> N(2),
    ]);
}
