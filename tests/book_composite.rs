use grw::prelude::*;
use grw::composite::{Included, KindOf, Kinded, Removed, TypeSet};
use grw::edge::{Anydir, Dir};
use grw::graph::{Graph, MGraph};
use grw::modify::error::{apply, Apply, Modify};
use grw::id;

#[derive(Debug, Clone, PartialEq)]
pub struct Signs {
    pub since: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Uses {
    pub service: u32,
}

#[derive(Debug, Clone, PartialEq, Part)]
pub enum Rel {
    Signs(Signs),
    Uses(Uses),
}

pub type Link = TypeSet<Rel>;

#[derive(Debug, thiserror::Error)]
#[error("the link already holds another {0:?}")]
pub struct OtherPartHeld<K: std::fmt::Debug>(pub K);

#[derive(Debug, thiserror::Error)]
#[error("the link holds no {0:?}")]
pub struct NothingToExclude<K: std::fmt::Debug>(pub K);

#[derive(Debug, Clone, PartialEq)]
pub struct KeepHollow<P> {
    parts: Vec<P>,
}

impl<P: Kinded> KeepHollow<P> {
    pub fn is_hollow(&self) -> bool {
        self.parts.is_empty()
    }

    fn at(&self, kind: &P::Kind) -> Result<usize, usize> {
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
        self.at(kind).ok().map(|i| &self.parts[i])
    }

    fn with_part(&self, part: P) -> Result<Included<Self>, OtherPartHeld<P::Kind>> {
        match self.at(&part.kind()) {
            Ok(i) if self.parts[i] == part => Ok(Included::Held),
            Ok(_) => Err(OtherPartHeld(part.kind())),
            Err(i) => {
                let mut parts = self.parts.clone();
                parts.insert(i, part);
                Ok(Included::Added(KeepHollow { parts }))
            }
        }
    }

    fn without_part(&self, kind: &KindOf<Self>) -> Result<Removed<Self>, NothingToExclude<P::Kind>> {
        match self.at(kind) {
            Ok(i) => {
                let mut parts = self.parts.clone();
                parts.remove(i);
                Ok(Removed::Remains(KeepHollow { parts }))
            }
            Err(_) => Err(NothingToExclude(kind.clone())),
        }
    }
}

mod promoted {
    use grw::prelude::*;
    use grw::composite::Kinded;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Val, serde::Serialize, serde::Deserialize)]
    pub enum Role {
        Owner,
        Reader,
    }

    #[derive(Debug, Clone, PartialEq, Val, serde::Serialize, serde::Deserialize)]
    pub struct Grant {
        pub role: Role,
        pub since: u32,
    }

    impl Kinded for Grant {
        type Kind = Role;

        fn kind(&self) -> Role {
            self.role
        }
    }
}

#[test]
fn a_value_holds_parts() {
    let link = Link::from_part(Rel::from(Signs { since: 2023 }));
    let Ok(Included::Added(link)) = link.include(Uses { service: 7 }) else {
        panic!("Uses is new to the link");
    };

    assert_eq!(link.get::<Signs>(), Some(&Signs { since: 2023 }));
    assert_eq!(link.parts().count(), 2);
    assert_eq!(link.include(Uses { service: 7 }).ok(), Some(Included::Held));
    assert_eq!(link.include(Uses { service: 8 }).err().map(|refused| refused.kind), Some(RelKind::Uses));
    assert_eq!(link.exclude::<Uses>().ok(), Some(Removed::Remains(Link::from_part(Rel::from(Signs { since: 2023 })))));
}

#[test]
fn a_graph_literal_includes_parts() {
    let g = mgraph![<(), Dir<Link>>;
        N(0) & E().include(Signs { since: 2023 }).include(Uses { service: 7 }) >> N(1),
        n(1) & E().include(Uses { service: 9 }) >> N(2),
    ]
    .unwrap();
    assert_eq!(g.edge_count(), 2);

    let twice = mgraph![<(), Dir<Link>>;
        N(0) & E().include(Signs { since: 2023 }).include(Signs { since: 2024 }) >> N(1)
    ];
    assert!(twice.is_err());
}

#[test]
fn modify_includes_and_excludes_parts() {
    let mut g = mgraph![<(), Dir<Link>>;
        N(0) & E().include(Signs { since: 2023 }) >> N(1),
        n(1) & E().include(Uses { service: 9 }) >> N(2),
    ]
    .unwrap();

    modify!(g, [X(0) & E().include(Uses { service: 7 }) >> x(1)]).unwrap();
    modify!(g, [X(0) & E().include(Uses { service: 4 }) >> x(2)]).unwrap();
    modify!(g, [X(0) & E().exclude::<Signs>() >> x(1)]).unwrap();
    modify!(g, [X(1) & E().exclude_kind(RelKind::Uses) >> x(2)]).unwrap();
    modify!(g, [X(0) & !e() >> x(2)]).unwrap();

    assert_eq!(g.edge_count(), 1);
    let (_, link) = g.edges_between(id::N(0), id::N(1)).next().unwrap();
    assert_eq!(link.get::<Uses>(), Some(&Uses { service: 7 }));
    assert_eq!(link.get::<Signs>(), None);
}

#[test]
fn part_refusals_are_typed() {
    let mut g = mgraph![<(), Dir<Link>>; N(0) & E().include(Signs { since: 2023 }) >> N(1)].unwrap();

    match modify!(g, [X(0) & E().include(Signs { since: 1999 }) >> x(1)]).err() {
        Some(Modify::Apply(Apply::Edge(apply::Edge::PartRejected { refused, .. }))) => {
            assert_eq!(refused.of::<Link>().map(|r| r.kind), Some(RelKind::Signs));
        }
        other => panic!("expected PartRejected, got {other:?}"),
    }

    match modify!(g, [X(0) & E().exclude::<Uses>() >> x(1)]).err() {
        Some(Modify::Apply(Apply::Edge(apply::Edge::PartNotHeld { refused, .. }))) => {
            assert_eq!(refused.of::<Link>().map(|r| r.kind), Some(RelKind::Uses));
        }
        other => panic!("expected PartNotHeld, got {other:?}"),
    }

    let whole = Link::from_part(Rel::from(Uses { service: 7 }));
    let both = modify!(g, [X(0) & e().val(whole) >> x(1), x(0) & E().exclude::<Signs>() >> x(1)]);
    assert!(matches!(both.err(), Some(Modify::Apply(Apply::Edge(apply::Edge::PartConflict(_, _))))));
}

#[test]
fn a_downstream_combinator_keeps_a_hollow_link() {
    let mut g = mgraph![<(), Dir<KeepHollow<Rel>>>;
        N(0) & E().include(Signs { since: 2023 }) >> N(1)
    ]
    .unwrap();

    modify!(g, [X(0) & E().exclude::<Signs>() >> x(1)]).unwrap();

    assert_eq!(g.edge_count(), 1);
    assert!(g.edges_between(id::N(0), id::N(1)).all(|(_, link)| link.is_hollow()));
}

fn hub() -> MGraph<(), Dir<Link>> {
    mgraph![<(), Dir<Link>>;
        N(0) & E().include(Signs { since: 2021 }) >> N(1),
        n(0) & E().include(Signs { since: 2022 }) >> N(2),
        n(0) & E().include(Signs { since: 2023 }) >> N(3),
        n(1) & E().include(Signs { since: 2024 }) >> n(2),
        n(2) & E().include(Uses { service: 7 }) >> n(3),
    ]
    .unwrap()
}

#[test]
fn search_asks_a_link_for_its_parts() {
    let g = hub();

    let signed = search![&g, get(Mono) { N(0) & E().has::<Signs>() >> N(1) }].unwrap();
    assert_eq!(signed.iter().count(), 4);

    let recent = search![&g, get(Mono) {
        N(0) & E().has::<Signs>().test(|s: &Signs| s.since >= 2023) >> N(1)
    }]
    .unwrap();
    assert_eq!(recent.iter().count(), 2);

    let raw = search![&g, get(Mono) {
        N(0) & E().has_kind(RelKind::Uses).test(|p: &Rel| matches!(p, Rel::Uses(u) if u.service == 7)) >> N(1)
    }]
    .unwrap();
    assert_eq!(raw.iter().count(), 1);

    let whole = search![&g, get(Mono) { N(0) & E().test(|v: &Link| v.parts().count() == 1) >> N(1) }].unwrap();
    assert_eq!(whole.iter().count(), 5);
}

#[test]
fn a_negated_part_is_not_a_missing_link() {
    let g = hub();

    let no_uses = search![&g, get(Mono) {
        N(0) & E().has::<Signs>() >> N(1),
        n(0) & E().has::<Signs>() >> N(2),
        n(1) & !E().has::<Uses>() >> n(2)
    }]
    .unwrap();
    assert_eq!(no_uses.iter().count(), 5);

    let no_link = search![&g, get(Mono) {
        N(0) & E().has::<Signs>() >> N(1),
        n(0) & E().has::<Signs>() >> N(2),
        n(1) & !E() >> n(2)
    }]
    .unwrap();
    assert_eq!(no_link.iter().count(), 4);
}

#[test]
fn terms_on_one_pair_merge_into_one_constraint() {
    let g = hub();

    let merged = search![&g, get(Mono) {
        N(0) & E().has::<Signs>() >> N(1),
        n(0) & !E().has::<Uses>() >> n(1)
    }]
    .unwrap();
    assert_eq!(merged.query().edge_count(), 1);
    assert_eq!(merged.iter().count(), 4);

    let contradiction = search![&g, get(Mono) {
        N(0) & E().has::<Signs>() >> N(1),
        n(0) & !E().has::<Signs>() >> n(1)
    }];
    assert!(contradiction.is_err());
}

#[derive(Debug, Clone, PartialEq)]
pub struct Knows {
    pub since: u32,
}

#[derive(Debug, Clone, PartialEq, Part)]
pub enum Tie {
    Knows(Knows),
}

pub type Bond = TypeSet<Tie>;

#[test]
fn an_anydir_graph_takes_parts_in_the_slot_its_operator_picks() {
    let mut g = mgraph![<(), Anydir<Bond, Link>>;
        N(0) & E().include(Knows { since: 2019 }) ^ N(1),
        n(0) & E().include(Signs { since: 2023 }) >> n(1),
    ]
    .unwrap();
    assert_eq!(g.edge_count(), 2);

    modify!(g, [X(0) & E().include(Uses { service: 7 }) >> x(1), x(1) & E().exclude::<Knows>() ^ x(0)]).unwrap();
    assert_eq!(g.edge_count(), 1);

    let known = search![&g, get(Mono) { N(0) & E().has::<Knows>() ^ N(1) }].unwrap();
    assert_eq!(known.iter().count(), 0);
    let used = search![&g, get(Mono) { N(0) & E().has::<Signs>().has::<Uses>() >> N(1) }].unwrap();
    assert_eq!(used.iter().count(), 1);
}

#[test]
fn an_older_snapshot_loads_promoted() {
    use grw::graph::VGraph;
    use promoted::{Grant, Role};

    let old: MGraph<(), Dir<Grant>> = mgraph![<(), Dir<Grant>>;
        N(0) & E().val(Grant { role: Role::Owner, since: 2020 }) >> N(1)
    ]
    .unwrap();
    let path = std::env::temp_dir().join(format!("grw_book_promoted_{}.grw", std::process::id()));
    old.save(&path).unwrap();

    assert!(MGraph::<(), Dir<TypeSet<Grant>>>::load(&path).is_err());
    let g: MGraph<(), Dir<TypeSet<Grant>>> = MGraph::load_promoting(&path).unwrap();
    let (_, link) = g.edges_between(id::N(0), id::N(1)).next().unwrap();
    assert_eq!(link.parts().count(), 1);
    let v: VGraph<(), Dir<TypeSet<Grant>>> = VGraph::load_promoting(&path).unwrap();
    assert_eq!(v.edge_count(), 1);

    std::fs::remove_file(&path).unwrap();
}

fn rust_blocks(chapter: &str) -> Vec<String> {
    chapter
        .split("```rust\n")
        .skip(1)
        .map(|rest| rest.split("```").next().expect("a rust block is closed").trim_end().to_string())
        .collect()
}

fn indented(block: &str, by: usize) -> String {
    block
        .lines()
        .map(|line| match line.is_empty() {
            true => String::new(),
            false => format!("{}{line}", " ".repeat(by)),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn every_rust_block_of_the_chapter_is_compiled_here_verbatim() {
    let chapter = include_str!("../doc/site/src/composite-values.md");
    let source = include_str!("book_composite.rs");
    let blocks = rust_blocks(chapter);
    assert!(blocks.len() >= 10, "the chapter shows its snippets: {}", blocks.len());
    for block in blocks {
        assert!(
            [0, 4, 8].iter().any(|by| source.contains(&indented(&block, *by))),
            "not in tests/book_composite.rs verbatim:\n{block}"
        );
    }
}
