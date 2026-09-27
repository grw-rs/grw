use crate::composite::PartKind;
use crate::search::dsl::{Required, Tested};

pub(crate) type EdgePred<V> = Box<dyn Fn(&V) -> bool + Send + Sync>;

pub(crate) struct Term<V> {
    pub(crate) parts: Vec<Required<V>>,
    pub(crate) whole: Option<EdgePred<V>>,
}

fn conjunction<V: 'static>(whole: Vec<EdgePred<V>>, parts: Vec<EdgePred<V>>) -> EdgePred<V> {
    Box::new(move |v| whole.iter().all(|w| w(v)) && parts.iter().all(|p| p(v)))
}

pub(crate) enum Flaw {
    Repeated(PartKind),
    Contradictory,
}

impl<V: 'static> Term<V> {
    pub(crate) fn has_parts(&self) -> bool {
        !self.parts.is_empty()
    }

    pub(crate) fn is_plain(&self) -> bool {
        self.parts.is_empty() && self.whole.is_none()
    }

    fn into_pred(self) -> EdgePred<V> {
        let parts: Vec<EdgePred<V>> = self.parts.into_iter().map(|r| r.holds).collect();
        let whole: Vec<EdgePred<V>> = self.whole.into_iter().collect();
        conjunction(whole, parts)
    }

    fn bare_kinds(&self) -> Option<Vec<&PartKind>> {
        match (self.parts.is_empty(), &self.whole) {
            (false, None) => self
                .parts
                .iter()
                .map(|part| match part.tested {
                    Tested::Bare => Some(&part.kind),
                    Tested::Tested => None,
                })
                .collect(),
            _ => None,
        }
    }
}

pub(crate) struct Holds<V> {
    required: Vec<Required<V>>,
    whole: Vec<EdgePred<V>>,
    forbidden: Vec<Term<V>>,
}

pub(crate) struct Lacks<V> {
    terms: Vec<Term<V>>,
}

fn require<V>(required: &mut Vec<Required<V>>, parts: Vec<Required<V>>) -> Result<(), Flaw> {
    for part in parts {
        match required.iter().any(|held| held.kind.same(&part.kind)) {
            true => return Err(Flaw::Repeated(part.kind)),
            false => required.push(part),
        }
    }
    Ok(())
}

fn distinct<V>(term: Term<V>) -> Result<Term<V>, Flaw> {
    let mut parts = Vec::with_capacity(term.parts.len());
    require(&mut parts, term.parts)?;
    Ok(Term { parts, whole: term.whole })
}

impl<V: 'static> Holds<V> {
    pub(crate) fn new(earlier: Option<EdgePred<V>>, term: Term<V>) -> Result<Self, Flaw> {
        let mut holds = Holds { required: Vec::new(), whole: earlier.into_iter().collect(), forbidden: Vec::new() };
        holds.require(term)?;
        Ok(holds)
    }

    pub(crate) fn require(&mut self, term: Term<V>) -> Result<(), Flaw> {
        require(&mut self.required, term.parts)?;
        self.whole.extend(term.whole);
        Ok(())
    }

    pub(crate) fn forbid(&mut self, term: Term<V>) -> Result<(), Flaw> {
        self.forbidden.push(distinct(term)?);
        Ok(())
    }

    pub(crate) fn forbid_lacked(&mut self, lacks: Lacks<V>) {
        self.forbidden.extend(lacks.terms);
    }

    fn contradicts(&self) -> bool {
        self.forbidden.iter().any(|term| match term.bare_kinds() {
            Some(kinds) => kinds.iter().all(|kind| self.required.iter().any(|held| held.kind.same(kind))),
            None => false,
        })
    }

    pub(crate) fn into_pred(self) -> Result<EdgePred<V>, Flaw> {
        match self.contradicts() {
            true => Err(Flaw::Contradictory),
            false => {
                let Holds { required, whole, forbidden } = self;
                let parts: Vec<EdgePred<V>> = required.into_iter().map(|r| r.holds).collect();
                let held = conjunction(whole, parts);
                let forbidden: Vec<EdgePred<V>> = forbidden.into_iter().map(Term::into_pred).collect();
                Ok(Box::new(move |v| held(v) && !forbidden.iter().any(|t| t(v))))
            }
        }
    }
}

pub(crate) fn validated<V>(term: Term<V>) -> Result<(), Flaw> {
    distinct(term).map(|_| ())
}

impl<V: 'static> Lacks<V> {
    pub(crate) fn new(term: Term<V>) -> Result<Self, Flaw> {
        Ok(Lacks { terms: vec![distinct(term)?] })
    }

    pub(crate) fn lack(&mut self, term: Term<V>) -> Result<(), Flaw> {
        self.terms.push(distinct(term)?);
        Ok(())
    }

    pub(crate) fn into_pred(self) -> EdgePred<V> {
        let terms: Vec<EdgePred<V>> = self.terms.into_iter().map(Term::into_pred).collect();
        Box::new(move |v| terms.iter().any(|t| t(v)))
    }
}
