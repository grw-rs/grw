use grw::composite::Part;

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
