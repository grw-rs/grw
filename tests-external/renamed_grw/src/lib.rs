use grw2::composite::Part;
use grw2::layout::Val;

#[derive(Debug, Clone, Copy, PartialEq, Val)]
pub struct Weight {
    pub w: f64,
    pub active: bool,
}

#[grw2::repl]
impl Weight {
    pub fn weight(&self) -> f64 {
        self.w
    }

    pub fn heavy(w: f64) -> Self {
        Weight { w, active: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Val)]
pub enum Tier {
    Low = 1,
    High = 2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signs {
    pub since: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uses {
    pub service: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Part)]
pub enum Rel {
    Signs(Signs),
    Uses(Uses),
}

pub type Er = grw2::edge::Undir<()>;

pub fn triangle() -> grw2::MGraph<u8, Er> {
    grw2::mgraph![<u8, Er>; N(0).val(10u8) ^ (N(1).val(20u8) ^ (N(2).val(30u8) ^ n(0)))].unwrap()
}

pub fn edge_pattern() -> grw2::search::Pattern<u8, Er> {
    grw2::pattern![get(Mono) { N(a) ^ N(b) }].unwrap()
}

pub fn pinned_edge_count(g: &grw2::MGraph<u8, Er>, at: grw2::id::N) -> usize {
    grw2::search![g, get(Mono) { X(a = at) ^ N(b) }].unwrap().iter().count()
}
