use super::LocalId;
use super::Node;
use super::edge::Edge;
use super::node::{Bind, Exist, New, Translated};
use crate::modify::error::{Fragment, fragment};
use crate::graph;
use crate::id;
use rustc_hash::FxHashSet;

fn visit_node<NV, ER: graph::Edge>(
    node: &Node<NV, ER>,
    is_top: bool,
    new_defs: &mut Vec<LocalId>,
    new_refs: &mut Vec<LocalId>,
    exist_defs: &mut Vec<id::N>,
    translated_defs: &mut Vec<LocalId>,
    translated_refs: &mut Vec<LocalId>,
) {
    match node {
        Node::New(new, edges) => {
            match new {
                New::Add {
                    id: Some(local), ..
                } => {
                    new_defs.push(*local);
                }
                New::Add { id: None, .. } => {}
                New::Ref { id } => {
                    new_refs.push(*id);
                }
            }
            edges
                .iter()
                .for_each(|edge| visit_edge(edge, new_defs, new_refs, exist_defs, translated_defs, translated_refs));
        }
        Node::Exist(exist) => match exist {
            Exist::Bind { id, op, edges } => {
                match op {
                    Bind::Pass | Bind::Swap(_) => {
                        exist_defs.push(*id);
                    }
                    Bind::Ref => {
                        if is_top {
                            exist_defs.push(*id);
                        }
                    }
                }
                edges
                    .iter()
                    .for_each(|edge| visit_edge(edge, new_defs, new_refs, exist_defs, translated_defs, translated_refs));
            }
            Exist::Rem { .. } => {}
        },
        Node::Translated(tr) => match tr {
            Translated::Bind { id, op, edges } => {
                match op {
                    Bind::Pass | Bind::Swap(_) => {
                        translated_defs.push(*id);
                    }
                    Bind::Ref => {
                        if is_top {
                            translated_defs.push(*id);
                        } else {
                            translated_refs.push(*id);
                        }
                    }
                }
                edges
                    .iter()
                    .for_each(|edge| visit_edge(edge, new_defs, new_refs, exist_defs, translated_defs, translated_refs));
            }
            Translated::Rem { .. } => {}
        },
    }
}

fn visit_edge<NV, ER: graph::Edge>(
    edge: &Edge<NV, ER>,
    new_defs: &mut Vec<LocalId>,
    new_refs: &mut Vec<LocalId>,
    exist_defs: &mut Vec<id::N>,
    translated_defs: &mut Vec<LocalId>,
    translated_refs: &mut Vec<LocalId>,
) {
    let target = match edge {
        Edge::New { target, .. } => target,
        Edge::Exist { target, .. } => target,
    };
    visit_node(target, false, new_defs, new_refs, exist_defs, translated_defs, translated_refs);
}

pub(crate) fn check_fragment<NV, ER: graph::Edge>(ops: &[Node<NV, ER>]) -> Result<(), Fragment> {
    let mut new_defs = Vec::new();
    let mut new_refs = Vec::new();
    let mut exist_defs = Vec::new();
    let mut translated_defs = Vec::new();
    let mut translated_refs = Vec::new();
    let mut top_removes = Vec::new();
    let mut top_translated_removes = Vec::new();

    ops.iter().for_each(|op| match op {
        Node::Exist(Exist::Rem { id }) => top_removes.push(*id),
        Node::Translated(Translated::Rem { id }) => top_translated_removes.push(*id),
        _ => visit_node(op, true, &mut new_defs, &mut new_refs, &mut exist_defs, &mut translated_defs, &mut translated_refs),
    });

    let mut seen_new = FxHashSet::default();
    new_defs
        .iter()
        .copied()
        .find(|&local| !seen_new.insert(local))
        .map_or(Ok(()), |local| {
            Err(Fragment::Node(fragment::Node::DuplicateNew(local)))
        })?;

    new_refs
        .iter()
        .copied()
        .find(|local| !seen_new.contains(local))
        .map_or(Ok(()), |local| {
            Err(Fragment::Node(fragment::Node::UndefinedRef(local)))
        })?;

    let mut seen_exist = FxHashSet::default();
    exist_defs
        .iter()
        .copied()
        .find(|&id| !seen_exist.insert(id))
        .map_or(Ok(()), |id| {
            Err(Fragment::Node(fragment::Node::DuplicateExist(id)))
        })?;

    let remove_set: FxHashSet<id::N> = top_removes.iter().copied().collect();
    exist_defs
        .iter()
        .copied()
        .find(|id| remove_set.contains(id))
        .map_or(Ok(()), |id| {
            Err(Fragment::Node(fragment::Node::RemoveConflict(id)))
        })?;

    let mut seen_translated = FxHashSet::default();
    translated_defs
        .iter()
        .copied()
        .find(|&id| !seen_translated.insert(id))
        .map_or(Ok(()), |local| {
            Err(Fragment::Node(fragment::Node::DuplicateTranslated(local)))
        })?;

    translated_refs
        .iter()
        .copied()
        .find(|local| !seen_translated.contains(local))
        .map_or(Ok(()), |local| {
            Err(Fragment::Node(fragment::Node::UndefinedTranslatedRef(local)))
        })?;

    let translated_remove_set: FxHashSet<LocalId> = top_translated_removes.iter().copied().collect();
    translated_defs
        .iter()
        .copied()
        .find(|id| translated_remove_set.contains(id))
        .map_or(Ok(()), |local| {
            Err(Fragment::Node(fragment::Node::TranslatedRemoveConflict(local)))
        })?;

    Ok(())
}
