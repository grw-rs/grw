//! One test per invariant of a pin landing inside a ban cluster (see
//! `search::engine::seq::State::ban_backtrack`): before the fix, a ban-only
//! position that carried a pin was searched over every graph node instead of
//! the single pinned id, so the ban fired far more often than the pattern
//! asked for and valid matches were silently dropped.

use grw::graph::index::{Cardinality, IndexDecl, IndexName};
use grw::graph::{edge, MGraph};
use grw::search::{Pattern, Session};
use grw::{id, mgraph, pattern, search, Graph as _};
use rayon::iter::ParallelIterator;

type UER = edge::Undir<()>;
type DER = edge::Dir<()>;
type MU = MGraph<(), UER>;
type MD = MGraph<bool, DER>;

// ============================================================================
// § Fixture: three candidate "blockers", two named targets.
//
// p1 -> alice, p2 -> bob (only), p3 -> nobody. A pattern asking for "p not
// connected like this to alice" must keep p2 and p3 and drop only p1.
// ============================================================================

fn blocks_graph() -> MD {
    mgraph![
        N(0).val(true), N(1).val(true), N(2).val(true),
        N(3).val(false), N(4).val(false),
        n(0) >> n(3),
        n(1) >> n(4)
    ]
    .unwrap()
}

#[test]
fn negated_edge_to_a_shared_pin_bans_only_that_target() {
    let g = blocks_graph();
    let alice = id::N(3);
    let s = search![&g, get(Mono) { N(p).val(true) & !E() >> X(alice = alice) }].unwrap();
    let mut seen: Vec<u32> = s.iter().map(|m| *m.get(0u32).unwrap() as u32).collect();
    seen.sort_unstable();
    assert_eq!(seen, vec![1, 2], "!E must only reject the blocker whose edge points at the pin");
}

#[test]
fn negated_pinned_node_bans_only_that_target() {
    let g = blocks_graph();
    let alice = id::N(3);
    let s = search![&g,
        get(Mono) { N(p).val(true) >> (!X(alice = alice)).test(|_: &bool| true) }
    ]
    .unwrap();
    let mut seen: Vec<u32> = s.iter().map(|m| *m.get(0u32).unwrap() as u32).collect();
    seen.sort_unstable();
    assert_eq!(
        seen,
        vec![1, 2],
        "a pin on a banned context node must restrict the ban to that one node, \
         matching the equivalent !E spelling above"
    );
}

// ============================================================================
// § Explicit `ban(m) { .. }` block, sharing a get node, pin supplied through
// `Session::from_pattern` rather than an inline `= expr`.
//
// `pattern!` forbids `X`/context nodes outright, so the only way to hand a
// ban-only position an external id::N identity is a plain `N(z)` bound after
// the fact via `from_pattern`'s pins slice — this exercises the exact same
// `bindings` plumbing through a second, independent surface.
// ============================================================================

#[test]
fn explicit_ban_cluster_pin_via_from_pattern_bans_only_that_target() {
    let g = blocks_graph();
    let alice = id::N(3);
    let pat: Pattern<bool, DER> = pattern![
        get(Mono) { N(p).val(true) },
        ban(Mono) { n(p) >> N(z) }
    ]
    .unwrap();
    let p_lid = pat.lid("p").unwrap();

    let s = Session::from_pattern(pat, &g, &[("z", alice)]).unwrap();
    let mut seen: Vec<u32> = s.iter().map(|m| *m.get(p_lid).unwrap() as u32).collect();
    seen.sort_unstable();
    assert_eq!(
        seen,
        vec![1, 2],
        "an explicit ban cluster's pinned position must resolve to the one bound id, not the whole graph"
    );
}

// ============================================================================
// § De Morgan: one ban_only node shared by two edges ("not all together")
// versus two separate bans each pinned to the same node ("none of these").
//
// p --a--> five, q -x-> five (b absent). The node form only fires when BOTH
// edges are present at once, so with only `a` present it survives. The edge
// form fires if EITHER edge is present, so it rejects.
// ============================================================================

#[test]
fn not_all_together_vs_none_of_these() {
    let g: MU = mgraph![N(0), N(1), N(2), n(0) ^ n(2)].unwrap();
    let (p, q, five) = (id::N(0), id::N(1), id::N(2));

    let and_form: Pattern<(), UER> = pattern![
        get(Mono) { N(p), N(q) },
        ban(Mono) { n(p) ^ N(c), n(q) ^ n(c) }
    ]
    .unwrap();
    let and_session = Session::from_pattern(and_form, &g, &[("p", p), ("q", q), ("c", five)]).unwrap();
    assert_eq!(
        and_session.iter().count(),
        1,
        "one ban_only node grouping both edges only fires when both are present at once"
    );

    let or_form: Pattern<(), UER> = pattern![
        get(Mono) { N(p), N(q) },
        ban(Mono) { n(p) ^ N(c1) },
        ban(Mono) { n(q) ^ N(c2) }
    ]
    .unwrap();
    let or_session =
        Session::from_pattern(or_form, &g, &[("p", p), ("q", q), ("c1", five), ("c2", five)]).unwrap();
    assert_eq!(
        or_session.iter().count(),
        0,
        "two independent bans, each pinned to the same node, fire as soon as either edge is present"
    );
}

// ============================================================================
// § `.key(..)` and `.test(..)` on a pinned negated node.
// ============================================================================

const BY_MOD5: IndexName = IndexName("ban_pins_by_mod5");

fn by_mod5() -> IndexDecl<u32> {
    IndexDecl::new(BY_MOD5, Cardinality::Multi, |v: &u32| Some(*v % 5))
}

fn indexed_path(count: u32) -> MGraph<u32, UER> {
    let mut ops: Vec<grw::modify::Node<u32, UER>> =
        (0..count).map(|i| grw::modify::N::<u32, UER>(i).val(i).into()).collect();
    for i in 0..count.saturating_sub(1) {
        ops.push((grw::modify::n::<u32, UER>(i) ^ grw::modify::n::<u32, UER>(i + 1)).into());
    }
    let mut g: MGraph<u32, UER> = mgraph![].unwrap();
    g.modify(ops).unwrap();
    g.with_indices(vec![by_mod5()]).unwrap()
}

/// Independent, engine-free evaluator for the recurring shape used across
/// these tests: `get(Mono) { N(a) ^ N(b) }, ban(Mono) { n(a) ^ X(c = pin) }`.
/// Ordered adjacent pairs survive unless `pin` is free to take on the banned
/// role (distinct from both `a` and `b` under Mono) and `a` is adjacent to it.
fn brute_force_pin_ban_count<NV, ER: grw::graph::Edge>(g: &MGraph<NV, ER>, pin: id::N) -> usize {
    let ids: Vec<id::N> = g.iter_node_ids().collect();
    let mut count = 0usize;
    for &a in &ids {
        for &b in &ids {
            if a == b || !g.is_adjacent(*a, *b) {
                continue;
            }
            if pin == a || pin == b {
                count += 1;
                continue;
            }
            if !g.is_adjacent(*a, *pin) {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn key_and_test_agree_with_the_brute_force_count_on_a_pinned_ban() {
    // `% 5 == 0` matches several nodes (0, 5, 10, 15) besides the pin, so a
    // ban search that ignores the pin and falls back to the whole predicate
    // pool gives a different (wrong) answer than one that only tries node 5.
    let g = indexed_path(20);
    let five = id::N(5);
    let expected = brute_force_pin_ban_count(&g, five);

    let by_key = search![&g,
        get(Mono) { N(a) ^ N(b), n(a) ^ (!X(c = five)).key(BY_MOD5, 0u32) }
    ]
    .unwrap();
    let by_test = search![&g,
        get(Mono) { N(a) ^ N(b), n(a) ^ (!X(c = five)).test(|v: &u32| *v % 5 == 0) }
    ]
    .unwrap();

    assert_eq!(by_key.iter().count(), expected, ".key on a pinned negated node must match the brute-force count");
    assert_eq!(by_test.iter().count(), expected, ".test on a pinned negated node must match the brute-force count");
}

// ============================================================================
// § Brute-force oracle: 200 random small graphs, random pinned ban target.
// ============================================================================

struct Rng(u64);
impl Rng {
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
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

fn random_graph(rng: &mut Rng, node_count: u32) -> MU {
    let mut ops: Vec<grw::modify::Node<(), UER>> =
        (0..node_count).map(|i| grw::modify::N::<(), UER>(i).val(()).into()).collect();
    for i in 0..node_count {
        for j in (i + 1)..node_count {
            if rng.chance(2, 5) {
                ops.push((grw::modify::n::<(), UER>(i) ^ grw::modify::n::<(), UER>(j)).into());
            }
        }
    }
    let mut g: MU = mgraph![].unwrap();
    g.modify(ops).unwrap();
    g
}

#[test]
fn brute_force_oracle_over_random_pinned_bans() {
    let mut rng = Rng(0x5eed_5eed_5eed_5eedu64);
    for case in 0..200u64 {
        let node_count = 2 + (rng.below(7) as u32);
        let g = random_graph(&mut rng, node_count);
        let ids: Vec<id::N> = g.iter_node_ids().collect();
        let pin = ids[rng.below(ids.len() as u64) as usize];
        let expected = brute_force_pin_ban_count(&g, pin);

        let inline = search![&g,
            get(Mono) { N(p) ^ N(q), n(p) ^ (!X(c = pin)).test(|_: &()| true) }
        ]
        .unwrap();
        assert_eq!(
            inline.iter().count(),
            expected,
            "case {case}: shadow-ban shape diverged from the brute-force oracle (nodes={node_count}, pin={pin:?})"
        );

        let pat: Pattern<(), UER> = pattern![
            get(Mono) { N(p) ^ N(q) },
            ban(Mono) { n(p) ^ N(z) }
        ]
        .unwrap();
        let explicit = Session::from_pattern(pat, &g, &[("z", pin)]).unwrap();
        assert_eq!(
            explicit.iter().count(),
            expected,
            "case {case}: explicit-ban shape diverged from the brute-force oracle (nodes={node_count}, pin={pin:?})"
        );
    }
}

// ============================================================================
// § Seq / Par parity on every shape exercised above.
// ============================================================================

#[test]
fn seq_and_par_agree_on_every_pinned_ban_shape_above() {
    let g = blocks_graph();
    let alice = id::N(3);

    let edge_form = search![&g, get(Mono) { N(p).val(true) & !E() >> X(alice = alice) }].unwrap();
    assert_eq!(edge_form.iter().count(), edge_form.par_iter().count());

    let node_form = search![&g,
        get(Mono) { N(p).val(true) >> (!X(alice = alice)).test(|_: &bool| true) }
    ]
    .unwrap();
    assert_eq!(node_form.iter().count(), node_form.par_iter().count());

    let pat: Pattern<bool, DER> = pattern![
        get(Mono) { N(p).val(true) },
        ban(Mono) { n(p) >> N(z) }
    ]
    .unwrap();
    let explicit = Session::from_pattern(pat, &g, &[("z", alice)]).unwrap();
    assert_eq!(explicit.iter().count(), explicit.par_iter().count());

    let g2: MU = mgraph![N(0), N(1), N(2), n(0) ^ n(2)].unwrap();
    let (p, q, five) = (id::N(0), id::N(1), id::N(2));
    let and_form: Pattern<(), UER> = pattern![
        get(Mono) { N(p), N(q) },
        ban(Mono) { n(p) ^ N(c), n(q) ^ n(c) }
    ]
    .unwrap();
    let and_session = Session::from_pattern(and_form, &g2, &[("p", p), ("q", q), ("c", five)]).unwrap();
    assert_eq!(and_session.iter().count(), and_session.par_iter().count());

    let or_form: Pattern<(), UER> = pattern![
        get(Mono) { N(p), N(q) },
        ban(Mono) { n(p) ^ N(c1) },
        ban(Mono) { n(q) ^ N(c2) }
    ]
    .unwrap();
    let or_session =
        Session::from_pattern(or_form, &g2, &[("p", p), ("q", q), ("c1", five), ("c2", five)]).unwrap();
    assert_eq!(or_session.iter().count(), or_session.par_iter().count());
}
