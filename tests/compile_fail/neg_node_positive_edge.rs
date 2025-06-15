use grw::search::*;
use grw::graph::edge;

type ER = edge::Undir<()>;

fn main() {
    let _ = grw::search![<(), ER>;
        get(Morphism::Mono) {
            !N(0) & E() ^ N(1)
        }
    ];
}
