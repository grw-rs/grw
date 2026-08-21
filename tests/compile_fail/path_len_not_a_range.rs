// `.len(..)` requires a usize or a usize range; a string is not a path length.
fn main() {
    let _ = grw::search::path::Config::new(()).dfs().len("three");
}
