use grw::modify::*;

struct NoDef(u32);

fn main() {
    let _: Vec<Node<NoDef, grw::graph::edge::Dir<()>>> = grw::modify![
        N(1).val(NoDef(5)) >> N(2),
    ];
}
