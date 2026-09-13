use grw::pattern;
use grw::graph::edge;
use grw::search::Pattern;
fn main() {
    let _p: Result<Pattern<i32, edge::Undir<()>>, _> = pattern![get(Mono) { X(a) ^ N(b) }];
}
