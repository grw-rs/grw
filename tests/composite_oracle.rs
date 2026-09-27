use grw::composite::{Kinded, Part, TypeSet};
use grw::graph::dsl::{from_fragment, Op as GraphOp};
use grw::graph::{self, edge, MGraph};
use grw::search::dsl::{get, Op};
use grw::search::error::{Edge as EdgeError, Search as SearchError};
use grw::search::{query, Morphism, Session};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signs(pub u8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uses(pub u8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Audits(pub u8);

#[derive(Debug, Clone, PartialEq, Eq, Part)]
pub enum Rel {
    Signs(Signs),
    Uses(Uses),
    Audits(Audits),
}

type Link = TypeSet<Rel>;
type Der = edge::Dir<Link>;
type Aer = edge::Anydir<Link, Link>;

const KINDS: [RelKind; 3] = [RelKind::Signs, RelKind::Uses, RelKind::Audits];
const NODES: u32 = 5;
const CASES: u32 = 2000;

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

fn part(kind: RelKind, value: u8) -> Rel {
    match kind {
        RelKind::Signs => Rel::Signs(Signs(value)),
        RelKind::Uses => Rel::Uses(Uses(value)),
        RelKind::Audits => Rel::Audits(Audits(value)),
    }
}

fn value(rel: &Rel) -> u8 {
    match rel {
        Rel::Signs(Signs(v)) | Rel::Uses(Uses(v)) | Rel::Audits(Audits(v)) => *v,
    }
}

type Model = BTreeMap<(u32, u32), Vec<Rel>>;

fn random_parts(rng: &mut XorShift) -> Vec<Rel> {
    let mut parts: Vec<Rel> = Vec::new();
    for k in KINDS {
        if rng.chance(50) {
            parts.push(part(k, rng.below(3) as u8));
        }
    }
    if parts.is_empty() {
        let k = KINDS[rng.below(3) as usize];
        parts.push(part(k, rng.below(3) as u8));
    }
    parts
}

fn random_model(rng: &mut XorShift) -> Model {
    let mut model = Model::new();
    for a in 0..NODES {
        for b in 0..NODES {
            if a == b || !rng.chance(45) {
                continue;
            }
            model.insert((a, b), random_parts(rng));
        }
    }
    model
}

struct AnyModel {
    dir: Model,
    undir: Model,
}

impl AnyModel {
    fn link(&self, class: Class, a: u32, b: u32) -> Option<&Vec<Rel>> {
        match class {
            Class::Dir => self.dir.get(&(a, b)),
            Class::Undir => self.undir.get(&(a.min(b), a.max(b))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Dir,
    Undir,
}

fn random_any_model(rng: &mut XorShift) -> AnyModel {
    let dir = random_model(rng);
    let mut undir = Model::new();
    for a in 0..NODES {
        for b in (a + 1)..NODES {
            if rng.chance(45) {
                undir.insert((a, b), random_parts(rng));
            }
        }
    }
    AnyModel { dir, undir }
}

fn build(model: &Model) -> MGraph<(), Der> {
    use grw::graph::dsl::{n, E, N};
    let mut ops: Vec<GraphOp<(), Der>> = (0..NODES).map(|i| N::<(), Der>(i).into()).collect();
    for (&(a, b), parts) in model {
        let mut edge = E::<(), Der>().include(parts[0].clone());
        for p in &parts[1..] {
            edge = edge.include(p.clone());
        }
        ops.push((n::<(), Der>(a) & edge >> n(b)).into());
    }
    from_fragment(ops).unwrap()
}

fn build_any(model: &AnyModel) -> MGraph<(), Aer> {
    use grw::graph::dsl::{n, E, N};
    let mut ops: Vec<GraphOp<(), Aer>> = (0..NODES).map(|i| N::<(), Aer>(i).into()).collect();
    for (&(a, b), parts) in &model.dir {
        let mut edge = E::<(), Aer>().include(parts[0].clone());
        for p in &parts[1..] {
            edge = edge.include(p.clone());
        }
        ops.push((n::<(), Aer>(a) & edge >> n(b)).into());
    }
    for (&(a, b), parts) in &model.undir {
        let mut edge = E::<(), Aer>().include(parts[0].clone());
        for p in &parts[1..] {
            edge = edge.include(p.clone());
        }
        ops.push((n::<(), Aer>(a) & edge ^ n(b)).into());
    }
    from_fragment(ops).unwrap()
}

#[derive(Debug, Clone, Copy)]
struct Need {
    kind: RelKind,
    at_least: Option<u8>,
}

#[derive(Debug, Clone)]
struct Term {
    negated: bool,
    needs: Vec<Need>,
    typed_first: bool,
    class: Class,
}

impl Term {
    fn holds(&self, link: Option<&Vec<Rel>>) -> bool {
        let found = match link {
            None => false,
            Some(parts) => self.needs.iter().all(|need| {
                parts.iter().any(|p| p.kind() == need.kind && need.at_least.is_none_or(|t| value(p) >= t))
            }),
        };
        found != self.negated
    }

    fn is_plain(&self) -> bool {
        self.needs.is_empty()
    }

    fn repeats(&self) -> bool {
        self.needs.iter().enumerate().any(|(i, a)| self.needs[..i].iter().any(|b| b.kind == a.kind))
    }
}

#[derive(Debug, Clone, Copy)]
enum Shape {
    SamePair { second_reversed: bool },
    Path,
    NegatedPair { second_reversed: bool },
}

#[derive(Debug, Clone)]
struct Pattern {
    shape: Shape,
    first: Term,
    second: Term,
    plain: [Class; 2],
}

impl Pattern {
    fn one_slot(&self) -> bool {
        self.first.class == self.second.class
    }
}

#[derive(Debug, Clone, Copy)]
enum Draw {
    DirOnly,
    Either,
}

fn random_class(rng: &mut XorShift, draw: Draw) -> Class {
    match draw {
        Draw::DirOnly => Class::Dir,
        Draw::Either => match rng.chance(50) {
            true => Class::Dir,
            false => Class::Undir,
        },
    }
}

fn random_term(rng: &mut XorShift, negated: bool, draw: Draw) -> Term {
    let count = match rng.below(10) {
        0..=1 => 0,
        2..=7 => 1,
        _ => 2,
    };
    let needs = (0..count)
        .map(|_| Need {
            kind: KINDS[rng.below(3) as usize],
            at_least: match rng.chance(40) {
                true => Some(rng.below(3) as u8),
                false => None,
            },
        })
        .collect();
    let typed_first = rng.chance(50);
    Term { negated, needs, typed_first, class: random_class(rng, draw) }
}

fn random_pattern(rng: &mut XorShift, draw: Draw) -> Pattern {
    let plain = [random_class(rng, draw), random_class(rng, draw)];
    if rng.chance(20) {
        return Pattern {
            shape: Shape::NegatedPair { second_reversed: rng.chance(50) },
            first: random_term(rng, true, draw),
            second: random_term(rng, true, draw),
            plain,
        };
    }
    match rng.chance(60) {
        true => {
            let first_negated = rng.chance(25);
            let second_negated = !first_negated && rng.chance(50);
            Pattern {
                shape: Shape::SamePair { second_reversed: rng.chance(50) },
                first: random_term(rng, first_negated, draw),
                second: random_term(rng, second_negated, draw),
                plain,
            }
        }
        false => Pattern {
            shape: Shape::Path,
            first: random_term(rng, false, draw),
            second: random_term(rng, false, draw),
            plain,
        },
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Matches(Vec<Vec<u32>>),
    Repeated,
    Contradictory,
    Duplicate,
}

fn expected_compile(p: &Pattern) -> Option<Outcome> {
    match p.shape {
        Shape::Path => match p.first.repeats() || p.second.repeats() {
            true => Some(Outcome::Repeated),
            false => None,
        },
        Shape::NegatedPair { .. } => {
            if p.first.repeats() || p.second.repeats() {
                return Some(Outcome::Repeated);
            }
            match p.one_slot() && p.first.is_plain() && p.second.is_plain() {
                true => Some(Outcome::Duplicate),
                false => None,
            }
        }
        Shape::SamePair { .. } => {
            let (pos, other) = match p.first.negated {
                true => (&p.second, &p.first),
                false => (&p.first, &p.second),
            };
            if pos.repeats() || other.repeats() {
                return Some(Outcome::Repeated);
            }
            if !p.one_slot() {
                return None;
            }
            match other.negated {
                false => {
                    if pos.is_plain() && other.is_plain() {
                        return Some(Outcome::Duplicate);
                    }
                    let mut kinds: Vec<RelKind> = pos.needs.iter().chain(other.needs.iter()).map(|n| n.kind).collect();
                    let len = kinds.len();
                    kinds.sort();
                    kinds.dedup();
                    match kinds.len() == len {
                        true => None,
                        false => Some(Outcome::Repeated),
                    }
                }
                true => {
                    if other.is_plain() {
                        return Some(Outcome::Contradictory);
                    }
                    match other.needs.iter().all(|n| n.at_least.is_none() && pos.needs.iter().any(|m| m.kind == n.kind)) {
                        true => Some(Outcome::Contradictory),
                        false => None,
                    }
                }
            }
        }
    }
}

fn brute_force<'m>(link_at: impl Fn(Class, u32, u32) -> Option<&'m Vec<Rel>>, p: &Pattern) -> Outcome {
    match expected_compile(p) {
        Some(outcome) => outcome,
        None => {
            let arity = match p.shape {
                Shape::SamePair { .. } => 2,
                Shape::Path | Shape::NegatedPair { .. } => 3,
            };
            let negated_pair = matches!(p.shape, Shape::NegatedPair { .. });
            let mut out = Vec::new();
            for a in 0..NODES {
                for b in 0..NODES {
                    if a == b {
                        continue;
                    }
                    match arity {
                        2 => {
                            if p.first.holds(link_at(p.first.class, a, b)) && p.second.holds(link_at(p.second.class, a, b)) {
                                out.push(vec![a, b]);
                            }
                        }
                        _ => {
                            for c in 0..NODES {
                                if c == a || c == b {
                                    continue;
                                }
                                let fits = match negated_pair {
                                    false => {
                                        p.first.holds(link_at(p.first.class, a, b))
                                            && p.second.holds(link_at(p.second.class, b, c))
                                    }
                                    true => {
                                        link_at(p.plain[0], a, b).is_some()
                                            && link_at(p.plain[1], a, c).is_some()
                                            && p.first.holds(link_at(p.first.class, b, c))
                                            && p.second.holds(link_at(p.second.class, b, c))
                                    }
                                };
                                if fits {
                                    out.push(vec![a, b, c]);
                                }
                            }
                        }
                    }
                }
            }
            out.sort();
            Outcome::Matches(out)
        }
    }
}

type SearchEdge<ER> = grw::search::dsl::edge::Edge<grw::search::dsl::Focus<Rel, Rel>, (), ER>;

fn focus<ER: graph::Edge>(needs: &[Need]) -> SearchEdge<ER> {
    use grw::prelude::*;
    use grw::search::dsl::E;
    let mut edge = E::<(), ER>().has_kind(needs[0].kind);
    edge = at_least(edge, needs[0].at_least);
    for need in &needs[1..] {
        edge = at_least(edge.has_kind(need.kind), need.at_least);
    }
    edge
}

fn at_least<ER: graph::Edge>(edge: SearchEdge<ER>, at_least: Option<u8>) -> SearchEdge<ER> {
    match at_least {
        None => edge,
        Some(t) => edge.test(move |p: &Rel| value(p) >= t),
    }
}

macro_rules! connect {
    ($a:expr, $edge:expr, $b:expr, $reversed:expr) => {{
        use grw::search::dsl::n;
        match $reversed {
            false => (n::<(), Der>($a) & $edge >> n($b)).into(),
            true => (n::<(), Der>($b) & $edge << n($a)).into(),
        }
    }};
}

fn edge_op(a: u32, b: u32, term: &Term, reversed: bool) -> Op<(), Der> {
    use grw::prelude::*;
    use grw::search::dsl::E;
    let signs_first = matches!(term.needs.first(), Some(Need { kind: RelKind::Signs, at_least: None })) && term.typed_first;
    match (term.negated, term.needs.len(), signs_first) {
        (false, 0, _) => connect!(a, E::<(), Der>(), b, reversed),
        (true, 0, _) => connect!(a, !E::<(), Der>(), b, reversed),
        (false, 1, true) => connect!(a, E::<(), Der>().has::<Signs>(), b, reversed),
        (true, 1, true) => connect!(a, !E::<(), Der>().has::<Signs>(), b, reversed),
        (false, _, _) => connect!(a, focus::<Der>(&term.needs), b, reversed),
        (true, _, _) => connect!(a, !focus::<Der>(&term.needs), b, reversed),
    }
}

macro_rules! connect_any {
    ($a:expr, $edge:expr, $b:expr, $class:expr, $reversed:expr) => {{
        use grw::search::dsl::n;
        match ($class, $reversed) {
            (Class::Dir, false) => (n::<(), Aer>($a) & $edge >> n($b)).into(),
            (Class::Dir, true) => (n::<(), Aer>($b) & $edge << n($a)).into(),
            (Class::Undir, false) => (n::<(), Aer>($a) & $edge ^ n($b)).into(),
            (Class::Undir, true) => (n::<(), Aer>($b) & $edge ^ n($a)).into(),
        }
    }};
}

fn edge_op_any(a: u32, b: u32, term: &Term, reversed: bool) -> Op<(), Aer> {
    use grw::prelude::*;
    use grw::search::dsl::E;
    let class = term.class;
    let signs_first = matches!(term.needs.first(), Some(Need { kind: RelKind::Signs, at_least: None })) && term.typed_first;
    match (term.negated, term.needs.len(), signs_first) {
        (false, 0, _) => connect_any!(a, E::<(), Aer>(), b, class, reversed),
        (true, 0, _) => connect_any!(a, !E::<(), Aer>(), b, class, reversed),
        (false, 1, true) => connect_any!(a, E::<(), Aer>().has::<Signs>(), b, class, reversed),
        (true, 1, true) => connect_any!(a, !E::<(), Aer>().has::<Signs>(), b, class, reversed),
        (false, _, _) => connect_any!(a, focus::<Aer>(&term.needs), b, class, reversed),
        (true, _, _) => connect_any!(a, !focus::<Aer>(&term.needs), b, class, reversed),
    }
}

fn run<ER: graph::Edge<Val: Clone + 'static>>(
    g: &MGraph<(), ER>,
    p: &Pattern,
    edge_op: impl Fn(u32, u32, &Term, bool) -> Op<(), ER>,
) -> Outcome {
    use grw::search::dsl::N;
    let nodes: u32 = match p.shape {
        Shape::SamePair { .. } => 2,
        Shape::Path | Shape::NegatedPair { .. } => 3,
    };
    let mut ops: Vec<Op<(), ER>> = (0..nodes).map(|i| N::<(), ER>(i).into()).collect();
    match p.shape {
        Shape::SamePair { second_reversed } => {
            ops.push(edge_op(0, 1, &p.first, false));
            ops.push(edge_op(0, 1, &p.second, second_reversed));
        }
        Shape::Path => {
            ops.push(edge_op(0, 1, &p.first, false));
            ops.push(edge_op(1, 2, &p.second, false));
        }
        Shape::NegatedPair { second_reversed } => {
            let plain = |class| Term { negated: false, needs: Vec::new(), typed_first: false, class };
            ops.push(edge_op(0, 1, &plain(p.plain[0]), false));
            ops.push(edge_op(0, 2, &plain(p.plain[1]), false));
            ops.push(edge_op(1, 2, &p.first, false));
            ops.push(edge_op(1, 2, &p.second, second_reversed));
        }
    }
    match query::compile(vec![get(Morphism::Mono, ops)]) {
        Err(SearchError::Edge(EdgeError::PartRepeated { .. })) => Outcome::Repeated,
        Err(SearchError::Edge(EdgeError::Contradictory { .. })) => Outcome::Contradictory,
        Err(SearchError::Edge(EdgeError::Duplicate { .. })) => Outcome::Duplicate,
        Err(other) => panic!("unexpected compile error {other:?} for {p:?}"),
        Ok(compiled) => {
            let session = Session::from_search(compiled, g).unwrap();
            let mut out: Vec<Vec<u32>> =
                session.iter().map(|m| (0..nodes).map(|i| *m.get(i).unwrap()).collect()).collect();
            out.sort();
            Outcome::Matches(out)
        }
    }
}

struct Spread {
    matched: usize,
    empty: usize,
    repeated: usize,
    contradictory: usize,
    duplicate: usize,
    negated: usize,
}

impl Spread {
    fn new() -> Self {
        Spread { matched: 0, empty: 0, repeated: 0, contradictory: 0, duplicate: 0, negated: 0 }
    }

    fn count(&mut self, want: Outcome, p: &Pattern) {
        match want {
            Outcome::Matches(m) if m.is_empty() => self.empty += 1,
            Outcome::Matches(_) => {
                self.matched += 1;
                self.negated += usize::from(p.first.negated || p.second.negated);
            }
            Outcome::Repeated => self.repeated += 1,
            Outcome::Contradictory => self.contradictory += 1,
            Outcome::Duplicate => self.duplicate += 1,
        }
    }

    fn assert_wide(&self) {
        let spread = [self.matched, self.empty, self.repeated, self.contradictory, self.duplicate, self.negated];
        assert!(spread.iter().all(|&n| n >= 20), "matched, empty, repeated, contradictory, duplicate, negated: {spread:?}");
    }
}

#[test]
fn the_engine_agrees_with_brute_force_over_random_composite_links() {
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    let mut spread = Spread::new();
    for case in 0..CASES {
        let model = random_model(&mut rng);
        let g = build(&model);
        let p = random_pattern(&mut rng, Draw::DirOnly);
        let want = brute_force(
            |class, a, b| match class {
                Class::Dir => model.get(&(a, b)),
                Class::Undir => panic!("the Dir oracle draws directed terms only"),
            },
            &p,
        );
        let got = run(&g, &p, edge_op);
        assert_eq!(got, want, "case {case}: pattern {p:?} over {model:?}");
        spread.count(want, &p);
    }
    spread.assert_wide();
}

#[test]
fn the_engine_agrees_with_brute_force_over_random_anydir_links_with_a_slot_class_per_term() {
    let mut rng = XorShift(0x2545_F491_4F6C_DD1D);
    let mut spread = Spread::new();
    let mut terms = [0usize; 2];
    let mut mixed = 0usize;
    for case in 0..CASES {
        let model = random_any_model(&mut rng);
        let g = build_any(&model);
        let p = random_pattern(&mut rng, Draw::Either);
        let want = brute_force(|class, a, b| model.link(class, a, b), &p);
        let got = run(&g, &p, edge_op_any);
        assert_eq!(got, want, "case {case}: pattern {p:?} over dir {:?} undir {:?}", model.dir, model.undir);
        terms[p.first.class as usize] += 1;
        terms[p.second.class as usize] += 1;
        mixed += usize::from(!p.one_slot());
        spread.count(want, &p);
    }
    spread.assert_wide();
    assert!(terms.iter().all(|&n| n >= 400), "dir, undir terms: {terms:?}");
    assert!(mixed >= 400, "patterns mixing both classes: {mixed}");
}
