use grw::modify::*;

struct NoDef(u32);

fn main() {
    let _: Vec<Node<NoDef, grw::graph::edge::Undir<()>>> = grw::modify![
        N(1),
    ];
}
