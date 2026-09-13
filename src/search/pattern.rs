use crate::graph::{self, dsl::LocalId};
use crate::search::dsl::ClusterOps;
use crate::search::{error, query, Query, Search};

#[derive(Debug, Clone, Copy)]
pub struct Names(&'static [(&'static str, LocalId)]);

impl Names {
    pub const fn new(table: &'static [(&'static str, LocalId)]) -> Self {
        Names(table)
    }

    pub fn lid(&self, name: &str) -> Option<LocalId> {
        self.0.iter().find(|(n, _)| *n == name).map(|(_, l)| *l)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'static str, LocalId)> + '_ {
        self.0.iter().copied()
    }
}

pub struct Pattern<NV, ER: graph::Edge> {
    query: Query<NV, ER>,
    names: Names,
}

impl<NV, ER: graph::Edge> Pattern<NV, ER> {
    pub fn from_clusters(clusters: Vec<ClusterOps<NV, ER>>, names: Names) -> Result<Self, error::Search> {
        match query::compile(clusters)? {
            Search::Resolved(r) => Ok(Pattern { query: r.into_query(), names }),
            Search::Unresolved(u) => {
                let ids = u.translated_indices().iter().map(|&i| u.query().node_local_id(i)).collect();
                Err(error::Search::ContextInPattern(ids))
            }
        }
    }

    pub fn query(&self) -> &Query<NV, ER> {
        &self.query
    }

    pub fn names(&self) -> &Names {
        &self.names
    }

    pub fn lid(&self, name: &str) -> Result<LocalId, error::Search> {
        self.names.lid(name).ok_or_else(|| error::Search::UnknownName(name.to_string()))
    }

    pub fn into_parts(self) -> (Query<NV, ER>, Names) {
        (self.query, self.names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::edge;
    use crate::search::dsl::*;

    fn two_node_clusters() -> Vec<ClusterOps<i32, edge::Undir<()>>> {
        vec![get(Mono, vec![(N(0) ^ N(1)).into()])]
    }

    #[test]
    fn from_clusters_resolved_keeps_names() {
        static T: [(&str, LocalId); 2] = [("a", LocalId(0)), ("b", LocalId(1))];
        let p = Pattern::from_clusters(two_node_clusters(), Names::new(&T)).unwrap();
        assert_eq!(p.lid("a").unwrap(), LocalId(0));
        assert_eq!(p.lid("b").unwrap(), LocalId(1));
        assert_eq!(p.query().node_count(), 2);
        assert!(matches!(p.lid("zz"), Err(error::Search::UnknownName(ref n)) if n == "zz"));
    }

    #[test]
    fn from_clusters_rejects_context_nodes() {
        static T: [(&str, LocalId); 0] = [];
        let clusters: Vec<ClusterOps<i32, edge::Undir<()>>> = vec![get(Mono, vec![(X(0) ^ N(1)).into()])];
        let err = Pattern::from_clusters(clusters, Names::new(&T)).err().unwrap();
        assert!(matches!(err, error::Search::ContextInPattern(ref ids) if ids == &[LocalId(0)]));
    }

    #[test]
    fn names_iter_order_is_table_order() {
        static T: [(&str, LocalId); 2] = [("b", LocalId(1)), ("a", LocalId(0))];
        let got: Vec<_> = Names::new(&T).iter().collect();
        assert_eq!(got, vec![("b", LocalId(1)), ("a", LocalId(0))]);
    }
}
