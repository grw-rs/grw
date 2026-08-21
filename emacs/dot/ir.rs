pub struct Pattern {
    pub clusters: Vec<Cluster>,
}

pub struct Cluster {
    pub decision: Decision,
    pub morphism: Morphism,
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Get,
    Ban,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Morphism {
    Iso,
    SubIso,
    EpiMono,
    Mono,
    Epi,
    Homo,
}

pub struct Stmt {
    pub node: Node,
    pub edges: Vec<Edge>,
}

pub struct Node {
    pub kind: NodeKind,
    pub id: Option<u32>,
    pub negated: bool,
    pub has_val: bool,
    pub has_pred: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Free,
    FreeRef,
    Context,
    ContextRef,
}

pub struct Edge {
    pub dir: Dir,
    pub negated: bool,
    pub has_edge_val: bool,
    pub has_edge_pred: bool,
    pub target: Stmt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Undirected,
    Forward,
    Backward,
    Any,
}

impl std::fmt::Display for Morphism {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Morphism::Iso => f.write_str("Iso"),
            Morphism::SubIso => f.write_str("SubIso"),
            Morphism::EpiMono => f.write_str("EpiMono"),
            Morphism::Mono => f.write_str("Mono"),
            Morphism::Epi => f.write_str("Epi"),
            Morphism::Homo => f.write_str("Homo"),
        }
    }
}

impl std::fmt::Display for Decision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Decision::Get => f.write_str("get"),
            Decision::Ban => f.write_str("ban"),
        }
    }
}

impl std::fmt::Display for Dir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Dir::Undirected => f.write_str("^"),
            Dir::Forward => f.write_str(">>"),
            Dir::Backward => f.write_str("<<"),
            Dir::Any => f.write_str("%"),
        }
    }
}
