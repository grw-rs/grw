use super::{Composite, Included, KindPresent, Kinded, NotCanonical, NotHeld, Removed, TypeSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
enum Rel {
    Signs(u32),
    Uses(String),
    Owns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RelKind {
    Signs,
    Uses,
    Owns,
}

impl Kinded for Rel {
    type Kind = RelKind;

    fn kind(&self) -> RelKind {
        match self {
            Rel::Signs(_) => RelKind::Signs,
            Rel::Uses(_) => RelKind::Uses,
            Rel::Owns => RelKind::Owns,
        }
    }
}

fn added<C: Composite>(included: Included<C>) -> C {
    match included {
        Included::Added(c) => c,
        Included::Held => panic!("expected a new part to be added"),
    }
}

fn remains<C: Composite>(removed: Removed<C>) -> C {
    match removed {
        Removed::Remains(c) => c,
        Removed::Vacant => panic!("expected parts to remain"),
    }
}

fn set_of(parts: Vec<Rel>) -> TypeSet<Rel> {
    let mut iter = parts.into_iter();
    let first = iter.next().expect("at least one part");
    iter.fold(TypeSet::from_part(first), |set, part| {
        added(set.with_part(part).expect("distinct kinds"))
    })
}

fn collected(set: &TypeSet<Rel>) -> Vec<Rel> {
    set.parts().cloned().collect()
}

#[test]
fn promotion_then_parts_yields_the_part() {
    let set = TypeSet::from_part(Rel::Signs(7));
    assert_eq!(collected(&set), vec![Rel::Signs(7)]);
    assert_eq!(set.part(&RelKind::Signs), Some(&Rel::Signs(7)));
    assert_eq!(set.part(&RelKind::Uses), None);
}

#[test]
fn with_then_without_restores() {
    let before = TypeSet::from_part(Rel::Uses("svc".into()));
    let grown = added(before.with_part(Rel::Signs(3)).expect("new kind"));
    assert_eq!(grown.part(&RelKind::Signs), Some(&Rel::Signs(3)));
    let after = remains(grown.without_part(&RelKind::Signs).expect("held kind"));
    assert_eq!(after, before);
}

#[test]
fn canonical_order_is_independent_of_insertion_order() {
    let a = set_of(vec![Rel::Owns, Rel::Uses("x".into()), Rel::Signs(1)]);
    let b = set_of(vec![Rel::Signs(1), Rel::Owns, Rel::Uses("x".into())]);
    let c = set_of(vec![Rel::Uses("x".into()), Rel::Signs(1), Rel::Owns]);
    let expected = vec![Rel::Signs(1), Rel::Uses("x".into()), Rel::Owns];
    assert_eq!(collected(&a), expected);
    assert_eq!(a, b);
    assert_eq!(b, c);
    let bytes = |s: &TypeSet<Rel>| bincode::serialize(s).expect("serialize");
    assert_eq!(bytes(&a), bytes(&b));
    assert_eq!(bytes(&b), bytes(&c));
}

#[test]
fn different_part_of_a_held_kind_is_rejected() {
    let set = TypeSet::from_part(Rel::Signs(1));
    assert_eq!(
        set.with_part(Rel::Signs(2)),
        Err(KindPresent { kind: RelKind::Signs })
    );
    assert_eq!(collected(&set), vec![Rel::Signs(1)]);
}

#[test]
fn equal_part_is_idempotent() {
    let set = set_of(vec![Rel::Signs(1), Rel::Owns]);
    assert_eq!(set.with_part(Rel::Signs(1)), Ok(Included::Held));
    assert_eq!(set.with_part(Rel::Owns), Ok(Included::Held));
}

#[test]
fn last_removal_is_vacant() {
    let set = TypeSet::from_part(Rel::Owns);
    assert_eq!(set.without_part(&RelKind::Owns), Ok(Removed::Vacant));
    let two = set_of(vec![Rel::Owns, Rel::Signs(4)]);
    let one = remains(two.without_part(&RelKind::Owns).expect("held"));
    assert_eq!(one.without_part(&RelKind::Signs), Ok(Removed::Vacant));
}

#[test]
fn removing_a_missing_kind_is_not_held() {
    let set = TypeSet::from_part(Rel::Owns);
    assert_eq!(
        set.without_part(&RelKind::Uses),
        Err(NotHeld { kind: RelKind::Uses })
    );
}

#[test]
fn serde_round_trip() {
    let set = set_of(vec![Rel::Uses("u".into()), Rel::Signs(9)]);
    let bytes = bincode::serialize(&set).expect("serialize");
    let back: TypeSet<Rel> = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(back, set);
    assert_eq!(bytes, bincode::serialize(&vec![Rel::Signs(9), Rel::Uses("u".into())]).expect("serialize"));
}

fn decoded(parts: Vec<Rel>) -> String {
    let bytes = bincode::serialize(&parts).expect("serialize");
    match bincode::deserialize::<TypeSet<Rel>>(&bytes) {
        Ok(set) => panic!("non-canonical input accepted: {set:?}"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn non_canonical_input_is_refused() {
    assert_eq!(decoded(vec![]), NotCanonical::<RelKind>::Empty.to_string());
    assert_eq!(
        decoded(vec![Rel::Owns, Rel::Signs(1)]),
        NotCanonical::OutOfOrder { before: RelKind::Owns, after: RelKind::Signs }.to_string()
    );
    assert_eq!(
        decoded(vec![Rel::Signs(1), Rel::Signs(2)]),
        NotCanonical::DuplicateKind { kind: RelKind::Signs }.to_string()
    );
}
