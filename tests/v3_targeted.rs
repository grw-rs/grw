use grw::{
    Id, NR, id,
    graph::{Undir0, edge},
    modify::{self, N, N_, X, e, n, x},
};

type UOp = modify::Node<(), edge::Undir<()>>;

fn degree_histogram(
    (ns, es): (Vec<(Id, ())>, Vec<(edge::undir::E<Id>, ())>),
) -> Vec<(Id, usize)> {
    use std::collections::BTreeMap;
    let mut deg: BTreeMap<Id, Id> = BTreeMap::new();
    for (nid, _) in ns {
        deg.entry(nid).or_insert(0);
    }
    for (edef, _) in es {
        let (nr, _): (NR<id::N>, edge::undir::Slot) = edef.into();
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

fn roundtrip(g: &Undir0) {
    let (ns, es) = g.to_vecs();
    let rebuilt: Undir0 = (ns, es).try_into().unwrap();
    assert_eq!(g.node_count(), rebuilt.node_count(), "node count mismatch");
    assert_eq!(g.edge_count(), rebuilt.edge_count(), "edge count mismatch");

    let actual_hist = degree_histogram(g.to_vecs());
    let expected_hist = degree_histogram(rebuilt.to_vecs());
    assert_eq!(
        actual_hist, expected_hist,
        "degree_histogram mismatch\n  actual:  {actual_hist:?}\n  expected: {expected_hist:?}"
    );
}

// ── Tier 1: Single-step modifications on tiny graphs ─────────────────────

#[test]
fn t1_1_add_isolated_node() {
    let mut g = Undir0::default();
    let ops: Vec<UOp> = vec![N_().into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
    assert_eq!(g.node_count(), 1);
    assert_eq!(g.edge_count(), 0);
}

#[test]
fn t1_2_add_edge_pair() {
    let mut g = Undir0::default();
    let ops: Vec<UOp> = vec![(N_() ^ N_()).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
}

#[test]
fn t1_3_build_triangle() {
    let mut g = Undir0::default();
    let ops: Vec<UOp> = vec![(N(1) ^ (N(2) ^ (N(3) ^ n(1)))).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
    assert_eq!(g.node_count(), 3);
    assert_eq!(g.edge_count(), 3);
}

#[test]
fn t1_4_build_star() {
    let mut g = Undir0::default();
    let ops: Vec<UOp> = vec![(N_() ^ N_() ^ N_() ^ N_()).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
    assert_eq!(g.node_count(), 4);
    assert_eq!(g.edge_count(), 3);
}

#[test]
fn t1_5_graft_onto_star_spoke() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(0, 2),
        edge::undir::E::U(0, 3),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(X(id::N(1)) ^ N_()).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t1_6_close_triangle_via_exist_edge() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(1, 2),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(X(id::N(0)) ^ x(id::N(2))).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
    assert_eq!(g.edge_count(), 3);
}

#[test]
fn t1_7_remove_spoke_from_star() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(0, 2),
        edge::undir::E::U(0, 3),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(!X(id::N(3))).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
    assert_eq!(g.node_count(), 3);
    assert_eq!(g.edge_count(), 2);
}

#[test]
fn t1_8_remove_node_from_triangle() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(1, 2),
        edge::undir::E::U(2, 0),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(!X(id::N(2))).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
}

#[test]
fn t1_9_remove_edge_from_triangle_with_tail() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(1, 2),
        edge::undir::E::U(2, 0),
        edge::undir::E::U(2, 3),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(X(id::N(0)) & !e() ^ x(id::N(1))).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
    assert_eq!(g.edge_count(), 3);
}

// ── Tier 2: Multi-structure interactions ─────────────────────────────────

#[test]
fn t2_1_graft_onto_core_member() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(1, 2),
        edge::undir::E::U(2, 0),
        edge::undir::E::U(2, 3),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(X(id::N(3)) ^ N_()).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t2_2_star_graft_creating_bridge() {
    let mut g: Undir0 = (
        14 as Id,
        vec![
            edge::undir::E::U(0, 1),
            edge::undir::E::U(0, 2),
            edge::undir::E::U(0, 3),
            edge::undir::E::U(10, 11),
            edge::undir::E::U(10, 12),
            edge::undir::E::U(10, 13),
        ],
    )
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(X(id::N(1)) ^ x(id::N(11))).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t2_3_two_cores_disconnect_bridge() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(1, 2),
        edge::undir::E::U(2, 0),
        edge::undir::E::U(3, 4),
        edge::undir::E::U(4, 5),
        edge::undir::E::U(5, 3),
        edge::undir::E::U(2, 3),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(X(id::N(2)) & !e() ^ x(id::N(3))).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t2_4_exist_edges_create_new_core() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(0, 2),
        edge::undir::E::U(0, 3),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (X(id::N(1)) ^ x(id::N(2)) ^ x(id::N(3))).into(),
        (X(id::N(2)) ^ x(id::N(3))).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t2_5_core_grows_via_graft() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(1, 2),
        edge::undir::E::U(2, 0),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (N_() ^ x(id::N(0)) ^ x(id::N(1)) ^ x(id::N(2))).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t2_6_core_splits_via_edge_removal() {
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
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (X(id::N(0)) & !e() ^ x(id::N(2))).into(),
        (X(id::N(1)) & !e() ^ x(id::N(3))).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

// ── Tier 3: Multi-step sequences ─────────────────────────────────────────

#[test]
fn t3_1_build_up_then_tear_down() {
    let mut g = Undir0::default();

    let ops: Vec<UOp> = vec![(N(1) ^ N(2) ^ N(3) ^ N(4)).into()];
    let r = g.modify(ops).unwrap();
    roundtrip(&g);
    assert_eq!(g.node_count(), 4);

    let spoke2 = r.new_node_ids[&modify::LocalId(2)];
    let spoke3 = r.new_node_ids[&modify::LocalId(3)];

    let ops: Vec<UOp> = vec![(X(spoke2) ^ (N_() ^ N_())).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (X(spoke2) ^ x(spoke3)).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(!X(spoke3)).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t3_2_incremental_growth() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(1, 2),
        edge::undir::E::U(2, 0),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(X(id::N(0)) ^ N_()).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(X(id::N(1)) ^ N_()).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);

    let nodes: Vec<(Id, ())> = g.to_vecs().0;
    let spoke_ids: Vec<Id> = nodes.iter().map(|&(id, _)| id).filter(|&id| id >= 3).collect();
    if spoke_ids.len() >= 2 {
        let ops: Vec<UOp> = vec![
            (X(id::N(spoke_ids[0])) ^ x(id::N(spoke_ids[1]))).into(),
        ];
        g.modify(ops).unwrap();
        roundtrip(&g);
    }
}

#[test]
fn t3_3_edge_churn() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(0, 2),
        edge::undir::E::U(0, 3),
        edge::undir::E::U(3, 4),
        edge::undir::E::U(4, 5),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(!X(id::N(1))).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(X(id::N(0)) ^ N_()).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![(X(id::N(2)) ^ x(id::N(5))).into()];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

// ── Tier 2b: Combined operations (multiple op types in one batch) ────────

#[test]
fn t2b_1_graft_plus_exist_edge_same_batch() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(0, 2),
        edge::undir::E::U(0, 3),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (X(id::N(1)) ^ x(id::N(2))).into(),
        (X(id::N(3)) ^ N_()).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t2b_2_removal_plus_graft_same_batch() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(0, 2),
        edge::undir::E::U(0, 3),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (!X(id::N(3))).into(),
        (X(id::N(1)) ^ N_()).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t2b_3_removal_plus_exist_edge_same_batch() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(0, 2),
        edge::undir::E::U(0, 3),
        edge::undir::E::U(0, 4),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (!X(id::N(4))).into(),
        (X(id::N(1)) ^ x(id::N(2))).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t2b_4_edge_removal_plus_graft_same_batch() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(1, 2),
        edge::undir::E::U(2, 0),
        edge::undir::E::U(2, 3),
        edge::undir::E::U(3, 4),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (X(id::N(0)) & !e() ^ x(id::N(1))).into(),
        (X(id::N(4)) ^ N_()).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t2b_5_multiple_grafts_same_batch() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(0, 2),
        edge::undir::E::U(0, 3),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (X(id::N(1)) ^ N_()).into(),
        (X(id::N(2)) ^ N_()).into(),
        (X(id::N(3)) ^ N_()).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t2b_6_graft_with_new_to_new_edges() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(0, 2),
        edge::undir::E::U(0, 3),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (X(id::N(1)) ^ (N(1) ^ (N(2) ^ (N(3) ^ n(1))))).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t2b_7_removal_plus_multiple_grafts_plus_exist_edge() {
    let mut g: Undir0 = (
        8 as Id,
        vec![
            edge::undir::E::U(0, 1),
            edge::undir::E::U(0, 2),
            edge::undir::E::U(0, 3),
            edge::undir::E::U(4, 5),
            edge::undir::E::U(4, 6),
            edge::undir::E::U(4, 7),
        ],
    )
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (!X(id::N(3))).into(),
        (X(id::N(1)) ^ x(id::N(5))).into(),
        (X(id::N(2)) ^ N_()).into(),
        (X(id::N(6)) ^ N_()).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t2b_8_graft_onto_core_plus_exist_edge_forming_larger_core() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(1, 2),
        edge::undir::E::U(2, 0),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (N_() ^ x(id::N(0)) ^ x(id::N(1)) ^ x(id::N(2))).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (X(id::N(0)) ^ N_()).into(),
        (X(id::N(1)) ^ N_()).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);
}

#[test]
fn t2b_9_two_step_grow_then_graft_plus_exist() {
    let mut g: Undir0 = vec![
        edge::undir::E::U(0, 1),
        edge::undir::E::U(0, 2),
        edge::undir::E::U(0, 3),
    ]
    .try_into()
    .unwrap();
    roundtrip(&g);

    let ops: Vec<UOp> = vec![
        (X(id::N(1)) ^ N_()).into(),
        (X(id::N(2)) ^ N_()).into(),
    ];
    g.modify(ops).unwrap();
    roundtrip(&g);

    let nodes_vec: Vec<(Id, ())> = g.to_vecs().0;
    let ids: Vec<Id> = nodes_vec.iter().map(|&(id, _)| id).collect();
    let new_ids: Vec<Id> = ids.iter().copied().filter(|&id| id >= 4).collect();
    if new_ids.len() >= 2 {
        let ops: Vec<UOp> = vec![
            (X(id::N(new_ids[0])) ^ x(id::N(new_ids[1]))).into(),
            (X(id::N(3)) ^ N_()).into(),
        ];
        g.modify(ops).unwrap();
        roundtrip(&g);
    }
}
