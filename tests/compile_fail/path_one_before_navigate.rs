// `.one()`/`.all()` order results by cost — only navigated paths have one.
fn main() {
    let _ = grw::search::path::Config::new(()).dfs().one();
}
