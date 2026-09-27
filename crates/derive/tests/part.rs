use grw::composite::{Composite, Included, Part, Removed, Typed};
use grw::TypeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Signs {
    since: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Uses {
    service: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Part)]
enum Rel {
    Signs(Signs),
    Uses(Uses),
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

#[test]
fn from_impl_promotes_each_variant() {
    let rel: Rel = Signs { since: 1 }.into();
    assert_eq!(rel, Rel::Signs(Signs { since: 1 }));
    let rel: Rel = Uses { service: "svc".into() }.into();
    assert_eq!(rel, Rel::Uses(Uses { service: "svc".into() }));
}

#[test]
fn typed_get_include_exclude_on_type_set() {
    let one = TypeSet::from_part(Rel::Signs(Signs { since: 1 }));
    assert_eq!(one.get::<Signs>(), Some(&Signs { since: 1 }));
    assert_eq!(one.get::<Uses>(), None);

    let two = added(one.include(Uses { service: "svc".into() }).expect("new kind"));
    assert_eq!(two.get::<Uses>(), Some(&Uses { service: "svc".into() }));

    let back = remains(two.exclude::<Uses>().expect("held"));
    assert_eq!(back, one);
}
