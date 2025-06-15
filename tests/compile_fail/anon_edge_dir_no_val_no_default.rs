use grw::modify::*;

struct NoDef(u32);

fn main() {
    let _: Vec<Node<(), grw::graph::edge::Dir<NoDef>>> = grw::modify![
        N(1) >> N(2),
    ];
}
