use grw::graph::edge;
use grw::search::Search;
use grw::search;

fn main() {
    let zero = grw::id::N(0);
    let _s: Result<Search<u8, edge::Undir<()>>, _> = search![get(Mono) { X(c = zero) ^ N(o) }];
}
