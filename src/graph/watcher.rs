use crate::id;
use crate::graph::dsl::LocalId;
use crate::graph::Edge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Continue,
    Pause,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BanVerdict {
    SharedEdgeFail,
    NoCandidate,
    Fired,
}

pub trait Watcher<NV, E: Edge> {
    const ACTIVE: bool = true;

    fn on_bind(&mut self, step: usize, pattern_node: LocalId, graph_node: id::N) -> Control;
    fn on_unbind(&mut self, step: usize, pattern_node: LocalId);
    fn on_edge_test(
        &mut self,
        pattern_edge: usize,
        src: id::N,
        tgt: id::N,
        exists: bool,
        pred_pass: bool,
        negated: bool,
    ) -> Control;
    fn on_ban_verdict(&mut self, cluster: usize, verdict: BanVerdict) -> Control;
    fn on_match(&mut self, mapping: &[(LocalId, id::N)]) -> Control;

    fn on_node_added(&mut self, id: id::N, val: &NV);
    fn on_node_removed(&mut self, id: id::N);
    fn on_node_changed(&mut self, id: id::N, val: &NV);
    fn on_edge_added(&mut self, id: id::E, n1: id::N, n2: id::N, slot: &E::Slot, val: &E::Val);
    fn on_edge_removed(&mut self, id: id::E);
    fn on_edge_changed(&mut self, id: id::E, val: &E::Val);
}

pub struct Silent;

impl<NV, E: Edge> Watcher<NV, E> for Silent {
    const ACTIVE: bool = false;

    #[inline(always)]
    fn on_bind(&mut self, _step: usize, _pn: LocalId, _gn: id::N) -> Control { Control::Continue }
    #[inline(always)]
    fn on_unbind(&mut self, _step: usize, _pn: LocalId) {}
    #[inline(always)]
    fn on_edge_test(&mut self, _pe: usize, _src: id::N, _tgt: id::N, _e: bool, _p: bool, _n: bool) -> Control { Control::Continue }
    #[inline(always)]
    fn on_ban_verdict(&mut self, _cluster: usize, _verdict: BanVerdict) -> Control { Control::Continue }
    #[inline(always)]
    fn on_match(&mut self, _mapping: &[(LocalId, id::N)]) -> Control { Control::Continue }

    #[inline(always)]
    fn on_node_added(&mut self, _id: id::N, _val: &NV) {}
    #[inline(always)]
    fn on_node_removed(&mut self, _id: id::N) {}
    #[inline(always)]
    fn on_node_changed(&mut self, _id: id::N, _val: &NV) {}
    #[inline(always)]
    fn on_edge_added(&mut self, _id: id::E, _n1: id::N, _n2: id::N, _slot: &E::Slot, _val: &E::Val) {}
    #[inline(always)]
    fn on_edge_removed(&mut self, _id: id::E) {}
    #[inline(always)]
    fn on_edge_changed(&mut self, _id: id::E, _val: &E::Val) {}
}

pub struct WatchedGraph<'a, NV, E: Edge, W: Watcher<NV, E>> {
    pub(crate) graph: &'a mut crate::graph::Graph<NV, E>,
    pub(crate) watcher: &'a mut W,
}

impl<'a, NV: Sync, E: Edge, W: Watcher<NV, E>> WatchedGraph<'a, NV, E, W> {
    pub fn graph(&self) -> &crate::graph::Graph<NV, E> {
        self.graph
    }

    pub fn graph_mut(&mut self) -> &mut crate::graph::Graph<NV, E> {
        self.graph
    }

    pub fn apply(
        &mut self,
        fragment: crate::modify::Fragment<NV, E, crate::modify::Checked>,
    ) -> Result<crate::modify::Modification<NV, E>, crate::modify::error::Apply> {
        let result = self.graph.apply(fragment)?;
        self.notify(&result);
        Ok(result)
    }

    pub fn modify(
        &mut self,
        ops: Vec<crate::modify::Node<NV, E>>,
    ) -> Result<crate::modify::Modification<NV, E>, crate::modify::error::Modify> {
        let result = self.graph.modify(ops)?;
        self.notify(&result);
        Ok(result)
    }

    fn notify(&mut self, result: &crate::modify::Modification<NV, E>) {
        for (&_local, &real_id) in &result.new_node_ids {
            if let Some(val) = self.graph.get(real_id) {
                self.watcher.on_node_added(real_id, val);
            }
        }

        for &(nid, ref _old_val) in &result.swapped_node_vals {
            if let Some(val) = self.graph.get(nid) {
                self.watcher.on_node_changed(nid, val);
            }
        }

        for &(eid, n1, n2, ref slot) in &result.added_edges {
            if let Some(rec) = self.graph.edges.get_by_id(eid) {
                self.watcher.on_edge_added(eid, n1, n2, slot, &rec.val);
            }
        }

        for &(eid, ref _nr, ref _slot, ref _val) in &result.removed_edges {
            self.watcher.on_edge_removed(eid);
        }

        for &(eid, ref _nr, ref _slot, ref _val) in &result.swapped_edge_vals {
            if let Some(rec) = self.graph.edges.get_by_id(eid) {
                self.watcher.on_edge_changed(eid, &rec.val);
            }
        }

        for &(nid, ref _old_val) in &result.removed_nodes {
            self.watcher.on_node_removed(nid);
        }
    }

    pub fn watcher(&mut self) -> &mut W {
        self.watcher
    }
}
