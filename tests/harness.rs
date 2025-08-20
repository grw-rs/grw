use grw::{
    Id, NR, id,
    graph::{self, Undir0, Dir0, edge},
    modify::{self, E, N, N_, X, e, n, x},
};
use std::time::{Duration, Instant};

fn degree_histogram<ER: graph::Edge>(
    (ns, es): (Vec<(Id, ())>, Vec<(ER::Def, ())>),
) -> Vec<(Id, usize)> {
    use std::collections::BTreeMap;
    let mut deg: BTreeMap<Id, Id> = BTreeMap::new();
    for (nid, _) in ns {
        deg.entry(nid).or_insert(0);
    }
    for (edef, _) in es {
        let (nr, _): (NR<id::N>, ER::Slot) = edef.into();
        let (n1, n2) = (**nr.n1(), **nr.n2());
        *deg.entry(n1).or_insert(0) += 1;
        if n1 != n2 {
            *deg.entry(n2).or_insert(0) += 1;
        }
    }
    let mut hist: BTreeMap<Id, usize> = BTreeMap::new();
    for (_, &d) in &deg {
        *hist.entry(d).or_insert(0) += 1;
    }
    let mut result: Vec<_> = hist.into_iter().collect();
    result.sort_by(|a, b| b.0.cmp(&a.0));
    result
}

type UOp = modify::Node<(), edge::Undir<()>>;
type DOp = modify::Node<(), edge::Dir<()>>;

fn undir_path(n: usize) -> Undir0 {
    let es: Vec<edge::undir::E<Id>> =
        (0..n as Id - 1).map(|i| edge::undir::E::U(i, i + 1)).collect();
    es.try_into().unwrap()
}

fn undir_star(k: usize) -> Undir0 {
    let es: Vec<edge::undir::E<Id>> =
        (1..=k as Id).map(|i| edge::undir::E::U(0, i)).collect();
    es.try_into().unwrap()
}

fn dir_path(n: usize) -> Dir0 {
    let es: Vec<edge::dir::E<Id>> =
        (0..n as Id - 1).map(|i| edge::dir::E::D(i, i + 1)).collect();
    es.try_into().unwrap()
}

fn undir_add_anon(n: usize) -> Vec<UOp> {
    (0..n).map(|_| N_().into()).collect()
}

fn undir_add_pairs(n: usize) -> Vec<UOp> {
    (0..n).map(|_| (N_() ^ N_()).into()).collect()
}

fn undir_hub_n_spurs(hub: Id, n: usize) -> Vec<UOp> {
    let op = (0..n).fold(X::<(), edge::Undir<()>>(hub), |acc, _| acc & (E() ^ N_()));
    vec![op.into()]
}

fn undir_one_spur_per_node(node_ids: impl Iterator<Item = Id>) -> Vec<UOp> {
    node_ids.map(|id| (X(id) & (E() ^ N_())).into()).collect()
}

fn dir_hub_n_fanout(hub: Id, n: usize) -> Vec<DOp> {
    let op = (0..n).fold(X::<(), edge::Dir<()>>(hub), |acc, _| acc & (E() >> N_()));
    vec![op.into()]
}

fn dir_add_anon(n: usize) -> Vec<DOp> {
    (0..n).map(|_| N_().into()).collect()
}

struct Run {
    label:   String,
    nodes:   usize,
    edges:   usize,
    ops:     usize,
    elapsed: Duration,
}

impl Run {
    fn print(&self) {
        println!(
            "{:<55}  nodes={:<6}  edges={:<6}  ops={:<6}  elapsed={:>9.2?}",
            self.label,
            self.nodes,
            self.edges,
            self.ops,
            self.elapsed,
        );
    }
}

fn run_undir(g: &mut Undir0, ops: Vec<UOp>, label: impl Into<String>) -> Run {
    let nodes = g.node_count();
    let edges = g.edge_count();
    let n_ops = ops.len();
    let t = Instant::now();
    g.modify(ops).unwrap();
    let elapsed = t.elapsed();
    Run { label: label.into(), nodes, edges, ops: n_ops, elapsed }
}

fn run_dir(g: &mut Dir0, ops: Vec<DOp>, label: impl Into<String>) -> Run {
    let nodes = g.node_count();
    let edges = g.edge_count();
    let n_ops = ops.len();
    let t = Instant::now();
    g.modify(ops).unwrap();
    let elapsed = t.elapsed();
    Run { label: label.into(), nodes, edges, ops: n_ops, elapsed }
}

fn roundtrip_undir(g: &Undir0) {
    let (ns, es) = g.to_vecs();
    let rebuilt: Undir0 = (ns, es).try_into().unwrap();
    assert_eq!(g.node_count(), rebuilt.node_count(), "roundtrip node count mismatch");
    assert_eq!(g.edge_count(), rebuilt.edge_count(), "roundtrip edge count mismatch");
    let actual = degree_histogram::<edge::Undir<()>>(g.to_vecs());
    let expected = degree_histogram::<edge::Undir<()>>(rebuilt.to_vecs());
    assert_eq!(actual, expected, "roundtrip degree_histogram mismatch");
}

fn roundtrip_dir(g: &Dir0) {
    let (ns, es) = g.to_vecs();
    let rebuilt: Dir0 = (ns, es).try_into().unwrap();
    assert_eq!(g.node_count(), rebuilt.node_count(), "roundtrip node count mismatch");
    assert_eq!(g.edge_count(), rebuilt.edge_count(), "roundtrip edge count mismatch");
    let actual = degree_histogram::<edge::Dir<()>>(g.to_vecs());
    let expected = degree_histogram::<edge::Dir<()>>(rebuilt.to_vecs());
    assert_eq!(actual, expected, "roundtrip degree_histogram mismatch");
}

#[test]
fn harness() {
    println!();
    println!(
        "{:<55}  {:<12}  {:<12}  {:<10}  {:<15}",
        "scenario", "nodes_before", "edges_before", "ops", "elapsed"
    );
    println!("{}", "-".repeat(110));

    for &n in &[100usize, 1_000, 10_000] {
        let mut g = Undir0::default();
        let r = run_undir(&mut g, undir_add_anon(n), format!("undir/empty → add {n} anon nodes"));
        r.print();
        roundtrip_undir(&g);
    }

    println!();

    for &n in &[100usize, 1_000, 10_000] {
        let mut g = Undir0::default();
        let r = run_undir(&mut g, undir_add_pairs(n), format!("undir/empty → add {n} new pairs"));
        r.print();
        roundtrip_undir(&g);
    }

    println!();

    for &n in &[100usize, 1_000, 10_000] {
        let mut g: Undir0 = vec![edge::undir::E::U(0, 1)].try_into().unwrap();
        let r = run_undir(&mut g, undir_hub_n_spurs(0, n), format!("undir/hub → 1 op × {n} spur edges"));
        r.print();
        roundtrip_undir(&g);
    }

    println!();

    for &k in &[100usize, 1_000, 5_000] {
        let mut g = undir_path(k);
        let r = run_undir(
            &mut g,
            undir_one_spur_per_node(0..k as Id),
            format!("undir/path({k}) → {k} ops × 1 spur each"),
        );
        r.print();
        roundtrip_undir(&g);
    }

    println!();

    for &(base, n) in &[(100usize, 1_000usize), (1_000, 10_000)] {
        let mut g = undir_star(base);
        let r = run_undir(
            &mut g,
            undir_hub_n_spurs(0, n),
            format!("undir/star({base}) → 1 op × {n} more spurs"),
        );
        r.print();
        roundtrip_undir(&g);
    }

    println!();

    for &n in &[100usize, 1_000, 10_000] {
        let mut g = Dir0::default();
        let r = run_dir(&mut g, dir_add_anon(n), format!("dir/empty → add {n} anon nodes"));
        r.print();
        roundtrip_dir(&g);
    }

    println!();

    for &(base, n) in &[(100usize, 1_000usize), (1_000, 10_000)] {
        let mut g = dir_path(base);
        let tail = base as Id - 1;
        let r = run_dir(
            &mut g,
            dir_hub_n_fanout(tail, n),
            format!("dir/path({base}) → 1 op × {n} fanout edges"),
        );
        r.print();
        roundtrip_dir(&g);
    }

    println!();
}

#[test]
fn roundtrip_core_incremental_k4() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(1, 2),
        edge::undir::E::U(2, 0),
    ]
    .try_into()
    .unwrap();
    roundtrip_undir(&g);

    let ops: Vec<UOp> = vec![
        (N_() ^ x(id::N(0)) ^ x(id::N(1)) ^ x(id::N(2))).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip_undir(&g);
}

#[test]
fn roundtrip_bridge_between_stars() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(0, 2),
        edge::undir::E::U(0, 3),
    ]
    .try_into()
    .unwrap();
    roundtrip_undir(&g);

    let ops: Vec<UOp> = vec![
        (X(id::N(3)) ^ (N_() ^ (N_() ^ N_() ^ N_() ^ N_()))).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip_undir(&g);
}

#[test]
fn roundtrip_terminal_from_star() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(0, 2),
        edge::undir::E::U(0, 3),
        edge::undir::E::U(0, 4),
    ]
    .try_into()
    .unwrap();
    roundtrip_undir(&g);

    let ops: Vec<UOp> = vec![
        (X(id::N(3)) ^ (N_() ^ N_())).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip_undir(&g);
}

#[test]
fn roundtrip_isolated_cycle() {
    let mut g = undir_path(5);
    roundtrip_undir(&g);

    let ops: Vec<UOp> = vec![
        (X(id::N(4)) ^ x(id::N(0))).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip_undir(&g);
}

#[test]
fn roundtrip_core_plus_core_cycle() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(1, 2),
        edge::undir::E::U(2, 0),
    ]
    .try_into()
    .unwrap();
    roundtrip_undir(&g);

    let ops: Vec<UOp> = vec![
        (X(id::N(2)) ^ (N_() ^ (N_() ^ (N_() ^ x(id::N(2)))))).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip_undir(&g);
}

#[test]
fn roundtrip_multi_step_growth() {
    let mut g = Undir0::default();

    g.modify(vec![N_().into(), N_().into(), N_().into()]).unwrap();
    roundtrip_undir(&g);

    g.modify(vec![
        (X(id::N(0)) ^ x(id::N(1)) ^ x(id::N(2))).into(),
    ])
    .unwrap();
    roundtrip_undir(&g);

    g.modify(vec![
        (X(id::N(0)) & (E() ^ N_()) & (E() ^ N_())).into(),
    ])
    .unwrap();
    roundtrip_undir(&g);

    let r = g
        .modify(vec![
            (N(1) ^ (N(2) ^ (N(3) ^ n(1)))).into(),
        ])
        .unwrap();
    let core_n = r.new_node_ids[&modify::LocalId(1)];
    roundtrip_undir(&g);

    g.modify(vec![
        (X(id::N(0)) ^ x(core_n)).into(),
    ])
    .unwrap();
    roundtrip_undir(&g);
}

#[test]
fn roundtrip_node_removal_star() {
    let mut g = undir_star(4);
    roundtrip_undir(&g);

    let ops: Vec<UOp> = vec![(!X(id::N(0))).into()];
    g.modify(ops).unwrap();
    roundtrip_undir(&g);
}

#[test]
fn roundtrip_edge_removal_path() {
    let mut g = undir_path(5);
    roundtrip_undir(&g);

    let ops: Vec<UOp> = vec![
        (X(id::N(2)) & !e() ^ x(id::N(3))).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip_undir(&g);
}

#[test]
fn roundtrip_node_removal_core() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(0, 2),
        edge::undir::E::U(0, 3),
        edge::undir::E::U(1, 2),
        edge::undir::E::U(1, 3),
        edge::undir::E::U(2, 3),
    ]
    .try_into()
    .unwrap();
    roundtrip_undir(&g);

    let ops: Vec<UOp> = vec![(!X(id::N(3))).into()];
    g.modify(ops).unwrap();
    roundtrip_undir(&g);
}

#[test]
fn roundtrip_directed_fan() {
    let mut g = dir_path(4);
    roundtrip_dir(&g);

    let ops: Vec<DOp> = vec![
        (X(id::N(1)) >> N_() >> N_()).into(),
        (N_() >> x(id::N(3))).into(),
        (N_() >> x(id::N(3))).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip_dir(&g);
}

#[test]
fn roundtrip_big_mix() {
    use edge::undir::E::U;

    let g: Undir0 = (
        36 as Id,
        vec![
            U(23, 24), U(23, 25), U(24, 25),
            U(30, 24), U(30, 25), U(30, 31), U(30, 32),
            U(24, 31), U(24, 32), U(25, 31), U(25, 32), U(31, 32),
            U(26, 27), U(26, 28), U(26, 29),
            U(27, 28), U(27, 29), U(28, 29),
            U(14, 15), U(14, 16), U(14, 17),
            U(20, 19), U(20, 18), U(20, 21), U(20, 22),
            U(2, 3),
            U(10, 11), U(11, 12), U(12, 13), U(13, 10),
            U(4, 5), U(5, 6),
            U(7, 8), U(8, 9),
            U(22, 4), U(20, 14), U(7, 25), U(9, 29),
            U(20, 23), U(7, 23),
            U(23, 33), U(24, 33), U(24, 34), U(25, 34),
            U(17, 35), U(26, 35),
            U(29, 32),
        ],
    )
        .try_into()
        .unwrap();

    roundtrip_undir(&g);
}
