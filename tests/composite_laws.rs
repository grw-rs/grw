use composite_downstream::{KeepHollow, Rel, RelKind, Signs, Uses};
use grw::composite::{Composite, Included, Kinded, Removed, Typed};
use grw::TypeSet;

fn signs(since: u32) -> Rel {
    Rel::Signs(Signs { since })
}

fn uses(service: &str) -> Rel {
    Rel::Uses(Uses { service: service.to_string() })
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
        Removed::Vacant => panic!("expected the value to remain"),
    }
}

fn kinds<C: Composite<Part = Rel>>(c: &C) -> Vec<RelKind> {
    c.parts().map(Kinded::kind).collect()
}

enum Emptied {
    Vacant,
    Hollow,
}

fn laws<C>(emptied: Emptied)
where
    C: Composite<Part = Rel> + PartialEq + std::fmt::Debug,
{
    let one = C::from_part(signs(1));
    assert_eq!(one.parts().cloned().collect::<Vec<_>>(), vec![signs(1)]);
    assert_eq!(one.part(&RelKind::Signs), Some(&signs(1)));
    assert_eq!(one.part(&RelKind::Uses), None);

    let two = added(one.with_part(uses("svc")).expect("new kind"));
    assert_eq!(remains(two.without_part(&RelKind::Uses).expect("held")), one);

    let other = added(C::from_part(uses("svc")).with_part(signs(1)).expect("new kind"));
    assert_eq!(other, two);
    assert_eq!(kinds(&two), vec![RelKind::Signs, RelKind::Uses]);

    assert!(matches!(two.with_part(signs(1)), Ok(Included::Held)));
    assert!(two.with_part(signs(2)).is_err());
    assert!(one.without_part(&RelKind::Uses).is_err());

    assert_eq!(two.get::<Signs>(), Some(&Signs { since: 1 }));
    assert_eq!(one.get::<Uses>(), None);
    assert_eq!(remains(two.exclude::<Uses>().expect("held")), one);
    assert_eq!(added(one.include(Uses { service: "svc".to_string() }).expect("new")), two);

    match (one.without_part(&RelKind::Signs).expect("held"), emptied) {
        (Removed::Vacant, Emptied::Vacant) => {}
        (Removed::Remains(hollow), Emptied::Hollow) => assert_eq!(hollow.parts().count(), 0),
        (Removed::Vacant, Emptied::Hollow) => panic!("expected a hollow value, got vacant"),
        (Removed::Remains(v), Emptied::Vacant) => panic!("expected vacant, got {v:?}"),
    }
}

#[test]
fn type_set_obeys_the_laws() {
    laws::<TypeSet<Rel>>(Emptied::Vacant);
}

#[test]
fn keep_hollow_obeys_the_laws() {
    laws::<KeepHollow<Rel>>(Emptied::Hollow);
}
