pub(crate) trait Edge {
    const HAS_PREDICATES: bool;
}

pub(crate) struct PlainEdges;
pub(crate) struct PredEdges;

impl Edge for PlainEdges { const HAS_PREDICATES: bool = false; }
impl Edge for PredEdges { const HAS_PREDICATES: bool = true; }

pub(crate) trait Ban {
    const ACTIVE: bool;
}

pub(crate) struct NoBans;
pub(crate) struct WithBans;

impl Ban for NoBans { const ACTIVE: bool = false; }
impl Ban for WithBans { const ACTIVE: bool = true; }

pub(crate) trait Emit {
    const COUNT_ONLY: bool;
}

pub(crate) struct Collect;
pub(crate) struct Count;

impl Emit for Collect { const COUNT_ONLY: bool = false; }
impl Emit for Count { const COUNT_ONLY: bool = true; }
