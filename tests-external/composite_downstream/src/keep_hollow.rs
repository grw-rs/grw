use grw::composite::{Composite, Included, KindOf, Kinded, Removed};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtherPartHeld<K> {
    pub kind: K,
}

impl<K: std::fmt::Debug> std::fmt::Display for OtherPartHeld<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the hollow value already holds another {:?}", self.kind)
    }
}

impl<K: std::fmt::Debug> std::error::Error for OtherPartHeld<K> {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NothingToExclude<K> {
    pub kind: K,
}

impl<K: std::fmt::Debug> std::fmt::Display for NothingToExclude<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the hollow value holds no {:?}", self.kind)
    }
}

impl<K: std::fmt::Debug> std::error::Error for NothingToExclude<K> {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeepHollow<P> {
    parts: Vec<P>,
}

impl<P: Kinded> KeepHollow<P> {
    pub fn is_hollow(&self) -> bool {
        self.parts.is_empty()
    }

    fn position(&self, kind: &P::Kind) -> Result<usize, usize> {
        self.parts.binary_search_by(|held| held.kind().cmp(kind))
    }
}

impl<P: Kinded + Clone + PartialEq> Composite for KeepHollow<P> {
    type Part = P;
    type IncludeRefused = OtherPartHeld<P::Kind>;
    type ExcludeRefused = NothingToExclude<P::Kind>;

    fn from_part(part: P) -> Self {
        KeepHollow { parts: vec![part] }
    }

    fn parts(&self) -> impl Iterator<Item = &P> {
        self.parts.iter()
    }

    fn part(&self, kind: &KindOf<Self>) -> Option<&P> {
        self.position(kind).ok().map(|at| &self.parts[at])
    }

    fn with_part(&self, part: P) -> Result<Included<Self>, OtherPartHeld<P::Kind>> {
        let kind = part.kind();
        match self.position(&kind) {
            Ok(at) if self.parts[at] == part => Ok(Included::Held),
            Ok(_) => Err(OtherPartHeld { kind }),
            Err(at) => {
                let mut parts = self.parts.clone();
                parts.insert(at, part);
                Ok(Included::Added(KeepHollow { parts }))
            }
        }
    }

    fn without_part(&self, kind: &KindOf<Self>) -> Result<Removed<Self>, NothingToExclude<P::Kind>> {
        match self.position(kind) {
            Ok(at) => {
                let mut parts = self.parts.clone();
                parts.remove(at);
                Ok(Removed::Remains(KeepHollow { parts }))
            }
            Err(_) => Err(NothingToExclude { kind: kind.clone() }),
        }
    }
}
