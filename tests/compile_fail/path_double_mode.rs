// A path has exactly one mode — traversal and navigation are mutually exclusive.
fn main() {
    let _ = grw::search::path::Config::new(())
        .dfs()
        .navigate(grw::search::path::Dijkstra::counted::<()>());
}
