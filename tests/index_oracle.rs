//! Differential index oracle, `vgraph_oracle`-style: 200 cases of 60 random
//! node ops on `MGraph<u32, Dir<()>>` with a unique index on `v % 97` and a
//! multi index on `v % 7`. After every accepted step, the expected map for
//! each index is derived fresh by scanning the graph's actual node values
//! (`iter_node_ids` + `node_val`) and compared against every possible key's
//! `index_hit` result; every refused step must leave the graph byte-equal
//! (`view()`) to before.

use std::collections::{BTreeSet, HashMap};

use grw::Graph as _;
use grw::graph::edge::Dir;
use grw::graph::index::{Cardinality, IndexDecl, IndexHit, IndexName, KeyBytes, KeyTag};
use grw::graph::{self, MGraph};
use grw::modify::{N, X};

type ER = Dir<()>;
type MG = graph::MDir<u32, ()>;

const UNIQUE: IndexName = IndexName("u_mod97");
const MULTI: IndexName = IndexName("m_mod7");
const UNIQUE_MOD: u32 = 97;
const MULTI_MOD: u32 = 7;
const VAL_RANGE: u64 = 300;

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
}

fn decls() -> Vec<IndexDecl<u32>> {
    vec![
        IndexDecl::new(UNIQUE, Cardinality::Unique, |v: &u32| Some(*v % UNIQUE_MOD)),
        IndexDecl::new(MULTI, Cardinality::Multi, |v: &u32| Some(*v % MULTI_MOD)),
    ]
}

fn view(g: &MG) -> Vec<(u32, u32)> {
    let mut v: Vec<(u32, u32)> = g.iter_node_ids().map(|n| (*n as u32, *g.node_val(n).unwrap())).collect();
    v.sort_unstable();
    v
}

/// Scans `g` itself (not any tracked bookkeeping) to build the map each
/// index should currently answer with.
fn expected_maps(g: &MG) -> (HashMap<u32, u32>, HashMap<u32, BTreeSet<u32>>) {
    let mut unique: HashMap<u32, u32> = HashMap::new();
    let mut multi: HashMap<u32, BTreeSet<u32>> = HashMap::new();
    for n in g.iter_node_ids() {
        let val = *g.node_val(n).unwrap();
        let id = *n as u32;
        assert!(
            unique.insert(val % UNIQUE_MOD, id).is_none(),
            "two live nodes share unique key {}",
            val % UNIQUE_MOD
        );
        multi.entry(val % MULTI_MOD).or_default().insert(id);
    }
    (unique, multi)
}

fn check(g: &MG) {
    let (unique, multi) = expected_maps(g);
    let tag = KeyTag::of::<u32>();

    for key in 0..UNIQUE_MOD {
        let expected = unique.get(&key).copied();
        let hit = g.index_hit(UNIQUE, &KeyBytes::of(&key), tag).unwrap();
        let actual = match hit {
            IndexHit::One(n) => Some(*n as u32),
            IndexHit::None => None,
            IndexHit::Many(_) => panic!("unique index returned Many for key {key}"),
        };
        assert_eq!(actual, expected, "unique key {key} mismatch");
    }

    for key in 0..MULTI_MOD {
        let expected: BTreeSet<u32> = multi.get(&key).cloned().unwrap_or_default();
        let hit = g.index_hit(MULTI, &KeyBytes::of(&key), tag).unwrap();
        let actual: BTreeSet<u32> = match hit {
            IndexHit::Many(set) => set.iter().map(|n| *n).collect(),
            IndexHit::None => BTreeSet::new(),
            IndexHit::One(_) => panic!("multi index returned One for key {key}"),
        };
        assert_eq!(actual, expected, "multi key {key} mismatch");
    }
}

enum Step {
    AddOne(u32),
    AddTwoSame(u32),
    Swap(grw::id::N, u32),
    Remove(grw::id::N),
}

fn random_step(rng: &mut Rng, existing: &[grw::id::N]) -> Step {
    let pick = rng.below(100);
    if pick < 10 {
        return Step::AddTwoSame(rng.below(VAL_RANGE) as u32);
    }
    if pick < 50 || existing.is_empty() {
        return Step::AddOne(rng.below(VAL_RANGE) as u32);
    }
    if pick < 80 {
        let a = existing[rng.below(existing.len() as u64) as usize];
        return Step::Swap(a, rng.below(VAL_RANGE) as u32);
    }
    let a = existing[rng.below(existing.len() as u64) as usize];
    Step::Remove(a)
}

fn step_ops(step: &Step) -> Vec<grw::modify::Node<u32, ER>> {
    match step {
        Step::AddOne(val) => vec![N::<u32, ER>(0).val(*val).into()],
        Step::AddTwoSame(val) => {
            vec![N::<u32, ER>(0).val(*val).into(), N::<u32, ER>(1).val(*val).into()]
        }
        Step::Swap(id, val) => vec![X::<u32, ER>(*id).val(*val).into()],
        Step::Remove(id) => vec![(!X::<u32, ER>(*id)).into()],
    }
}

#[test]
fn index_oracle() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut total_accepted: u64 = 0;
    let mut total_refused: u64 = 0;

    for _case in 0..200 {
        let mut g: MG = MGraph::default().with_indices(decls()).unwrap();

        for _step in 0..60 {
            let existing: Vec<grw::id::N> = g.iter_node_ids().collect();
            let step = random_step(&mut rng, &existing);
            let ops = step_ops(&step);
            let before = view(&g);

            match g.modify(ops) {
                Ok(_) => {
                    total_accepted += 1;
                    check(&g);
                }
                Err(_) => {
                    total_refused += 1;
                    assert_eq!(view(&g), before, "refused step must leave the graph untouched");
                }
            }
        }
    }

    println!("index_oracle: accepted={total_accepted} refused={total_refused}");
    assert_eq!(total_accepted, TOTAL_ACCEPTED, "regression: accepted-batch count drifted");
}

// Regression-locked: any change to op generation, the RNG stream, or the
// acceptance logic changes this count — recompute deliberately, don't tweak
// to make a red test pass.
const TOTAL_ACCEPTED: u64 = 10254;
