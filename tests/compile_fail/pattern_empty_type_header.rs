use grw::pattern;
use grw::search::Pattern;
fn main() {
    let _p: Result<Pattern<i32, grw::graph::edge::Undir<()>>, _> = pattern![<; get(Mono) { N(a) ^ N(b) }];
}
