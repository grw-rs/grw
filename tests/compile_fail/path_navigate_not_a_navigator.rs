// `.navigate(..)` requires a Navigator (Dijkstra/AStar); an integer is not one.
fn main() {
    let _ = grw::search::path::Config::new(()).navigate(42);
}
