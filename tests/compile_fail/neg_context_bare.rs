use grw::search::*;
use grw::graph::edge;

type ER = edge::Undir<()>;

fn main() {
    let _ = grw::search![<(), ER>;
        get(Morphism::Mono) {
            !X(0)
        }
    ];
}
