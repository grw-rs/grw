use grw::graph::edge;
use grw::graph::index::IndexName;
use grw::search;

const BY_VAL: IndexName = IndexName("by_val");

fn main() {
    let _ = search![<u32, edge::Undir<()>>;
        get(Mono) { N(0).key(BY_VAL, 1u32).key(BY_VAL, 2u32) ^ N(1) }
    ];
}
