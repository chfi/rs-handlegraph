use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::*,
    pathhandlegraph::*,
};

use crate::packedgraph::index::OneBasedIndex;
use crate::packedgraph::paths::StepPtr;
use crate::packedgraph::*;

use fnv::{FnvHashMap, FnvHashSet};

use rayon::prelude::*;

use super::simple_components;

#[allow(unused_imports)]
use log::{debug, error, info, trace};

/// Merge the handles in the provided slice to a single node, whose
/// sequence is the concatenation of the handles' sequences
/// left-to-right.
///
/// The handles must be in left-to-right order and share orientation,
/// and all paths that contain any of the handles must contain all
/// other handles, in the same order as in the slice (in either
/// direction).
fn concat_nodes(graph: &mut PackedGraph, handles: &[Handle]) -> Option<Handle> {
    if handles.len() < 2 {
        return None;
    }

    // TODO check paths, order, orientation

    let left = *handles.first()?;
    let right = *handles.last()?;

    // create the new node
    let new_seq: Vec<u8> = handles
        .iter()
        .flat_map(|handle| graph.sequence(*handle))
        .collect();

    let new_handle = graph.append_handle(&new_seq);

    // create the left neighbors
    let mut neighbors =
        graph.neighbors(left, Direction::Left).collect::<Vec<_>>();

    let mut left_neighbors: FnvHashSet<Handle> = FnvHashSet::default();

    for &other in neighbors.iter() {
        if other == right {
            left_neighbors.insert(new_handle);
        } else if other == left.flip() {
            left_neighbors.insert(new_handle.flip());
        } else {
            left_neighbors.insert(other);
        }
    }

    // create the right neighbors
    neighbors.clear();
    neighbors.extend(graph.neighbors(right, Direction::Right));

    let mut right_neighbors: FnvHashSet<Handle> = FnvHashSet::default();

    for other in neighbors {
        if other == left {
        } else if other == right.flip() {
            right_neighbors.insert(new_handle.flip());
        } else {
            right_neighbors.insert(other);
        }
    }

    for &other in left_neighbors.iter() {
        graph.create_edge(Edge(other, new_handle));
    }

    for &other in right_neighbors.iter() {
        graph.create_edge(Edge(new_handle, other));
    }

    // TODO update paths
    let mut to_rewrite: Vec<(PathId, StepPtr, StepPtr, bool)> = Vec::new();

    for (path_id, front_step) in
        graph.steps_on_handle(*handles.first().unwrap()).unwrap()
    {
        let runs_reverse = graph.path_handle_at_step(path_id, front_step)
            != handles.first().copied();

        let mut back_step = front_step;

        let last_step = if runs_reverse {
            let h = *handles.last().unwrap();
            h.flip()
        } else {
            *handles.last().unwrap()
        };

        loop {
            back_step = if runs_reverse {
                graph.path_prev_step(path_id, back_step).unwrap()
            } else {
                graph.path_next_step(path_id, back_step).unwrap()
            };

            if graph.path_handle_at_step(path_id, back_step).unwrap()
                == last_step
            {
                break;
            }
        }

        if runs_reverse {
            to_rewrite.push((path_id, back_step, front_step, true));
        } else {
            to_rewrite.push((path_id, front_step, back_step, false));
        }
    }

    for (path_id, from, to, rev) in to_rewrite {
        let new_seg = if rev { new_handle.flip() } else { new_handle };
        let to = graph
            .path_next_step(path_id, to)
            .unwrap_or_else(|| StepPtr::null());
        graph.path_rewrite_segment(path_id, from, to, &[new_seg]);
    }

    for other in left_neighbors {
        graph.remove_edge(Edge(other, left));
    }

    for other in right_neighbors {
        graph.remove_edge(Edge(right, other));
    }

    for window in handles.windows(2) {
        if let [this, next] = *window {
            graph.remove_edge(Edge(this, next));
        }
    }

    // remove the old nodes
    for &handle in handles.iter() {
        graph.remove_handle(handle);
    }

    Some(new_handle)
}

pub fn unchop(graph: &mut PackedGraph) {
    let node_rank: FnvHashMap<NodeId, f64> = graph
        .handles()
        .enumerate()
        .map(|(rank, handle)| (handle.id(), rank as f64))
        .collect();

    let components = simple_components(graph, 2);

    let to_merge: FnvHashSet<NodeId> = components
        .iter()
        .flat_map(|comp| comp.iter().map(|&h| h.id()))
        .collect();

    let mut ordered_handles: Vec<(f64, Handle)> = graph
        .handles_par()
        .filter_map(|handle| {
            if !to_merge.contains(&handle.id()) {
                Some((node_rank[&handle.id()], handle))
            } else {
                None
            }
        })
        .collect();

    for comp in components.iter() {
        if comp.len() >= 2 {
            let rank_sum: f64 = comp.iter().map(|h| node_rank[&h.id()]).sum();

            let rank_v = rank_sum / (comp.len() as f64);

            let n = concat_nodes(graph, &comp).unwrap();
            ordered_handles.push((rank_v, n));
        } else {
            for &handle in comp.iter() {
                ordered_handles.push((node_rank[&handle.id()], handle));
            }
        }
    }

    ordered_handles.par_sort_by(|a, b| b.partial_cmp(a).unwrap());

    let handle_order: Vec<Handle> =
        ordered_handles.into_iter().map(|(_, h)| h).collect();

    graph.apply_ordering(&handle_order);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hnd(x: u64) -> Handle {
        Handle::pack(x, false)
    }

    fn vec_hnd(v: Vec<u64>) -> Vec<Handle> {
        v.into_iter().map(hnd).collect::<Vec<_>>()
    }

    fn test_graph_1() -> PackedGraph {
        let mut graph = PackedGraph::default();

        let n1 = graph.append_handle(b"CAAATAAG");
        let n2 = graph.append_handle(b"A");
        let n3 = graph.append_handle(b"G");
        let n4 = graph.append_handle(b"T");
        let n5 = graph.append_handle(b"C");
        let n6 = graph.append_handle(b"TTG");

        graph.create_edge(Edge(n1, n2));
        graph.create_edge(Edge(n1, n5));
        graph.create_edge(Edge(n2, n3));
        graph.create_edge(Edge(n5, n6));
        graph.create_edge(Edge(n3, n4));
        graph.create_edge(Edge(n6, n4));

        let prep_path =
            |graph: &mut PackedGraph, name: &[u8], steps: Vec<Handle>| {
                let path = graph.create_path(name, false).unwrap();
                (path, steps)
            };

        let (_path_1, p_1_steps) =
            prep_path(&mut graph, b"path1", vec![n1, n2, n3, n4]);

        let (_path_2, p_2_steps) =
            prep_path(&mut graph, b"path2", vec![n1, n5, n6, n4]);

        let steps_vecs = vec![p_1_steps, p_2_steps];

        graph.zip_all_paths_mut_ctx(
            steps_vecs.into_par_iter(),
            |steps, _path_id, path| {
                steps
                    .into_iter()
                    .map(|h| path.append_handle(h))
                    .collect::<Vec<_>>()
            },
        );

        graph
    }

    #[test]
    fn concat_nodes_basic() {
        let mut graph = test_graph_1();

        let path_1 = graph.get_path_id(b"path1").unwrap();
        let path_2 = graph.get_path_id(b"path2").unwrap();

        let get_steps = |graph: &PackedGraph, path: PathId| {
            graph
                .get_path_ref(path)
                .map(|p| p.steps().map(|(_, s)| s.handle).collect::<Vec<_>>())
                .unwrap()
        };

        let p1_steps = get_steps(&graph, path_1);
        let p2_steps = get_steps(&graph, path_2);

        assert_eq!(p1_steps, vec_hnd(vec![1, 2, 3, 4]));
        assert_eq!(p2_steps, vec_hnd(vec![1, 5, 6, 4]));

        for i in 1..=6 {
            assert!(graph.has_node(i));
        }

        println!("Path 1: {:?}", p1_steps);
        println!("Path 2: {:?}", p2_steps);

        println!(" concatenating nodes ");

        let _n7 = concat_nodes(&mut graph, &[hnd(2), hnd(3)]);
        let _n8 = concat_nodes(&mut graph, &[hnd(5), hnd(6)]);

        assert!(graph.has_node(1));
        assert!(graph.has_node(4));
        assert!(graph.has_node(7));
        assert!(graph.has_node(8));

        assert!(!graph.has_node(2));
        assert!(!graph.has_node(3));
        assert!(!graph.has_node(5));
        assert!(!graph.has_node(6));

        let p1_steps = get_steps(&graph, path_1);
        let p2_steps = get_steps(&graph, path_2);

        assert_eq!(p1_steps, vec_hnd(vec![1, 7, 4]));
        assert_eq!(p2_steps, vec_hnd(vec![1, 8, 4]));

        println!("Path 1: {:?}", p1_steps);
        println!("Path 2: {:?}", p2_steps);
    }

    #[test]
    fn unchop_simple() {
        let mut graph = test_graph_1();

        let path_1 = graph.get_path_id(b"path1").unwrap();
        let path_2 = graph.get_path_id(b"path2").unwrap();

        let get_steps = |graph: &PackedGraph, path: PathId| {
            graph
                .get_path_ref(path)
                .map(|p| p.steps().map(|(_, s)| s.handle).collect::<Vec<_>>())
                .unwrap()
        };

        let get_handle = |graph: &PackedGraph, id: u64| {
            let lefts = graph
                .neighbors(hnd(id), Direction::Left)
                .map(|h| u64::from(h.id()))
                .collect::<Vec<_>>();
            let rights = graph
                .neighbors(hnd(id), Direction::Right)
                .map(|h| u64::from(h.id()))
                .collect::<Vec<_>>();
            (lefts, id, rights)
        };

        let p1_steps = get_steps(&graph, path_1);
        let p2_steps = get_steps(&graph, path_2);

        assert_eq!(p1_steps, vec_hnd(vec![1, 2, 3, 4]));
        assert_eq!(p2_steps, vec_hnd(vec![1, 5, 6, 4]));

        for i in 1..=6 {
            println!("node {}: {:?}", i, get_handle(&graph, i as u64));
            assert!(graph.has_node(i));
        }

        println!("Path 1: {:?}", p1_steps);
        println!("Path 2: {:?}", p2_steps);

        println!(" unchopping ");

        unchop(&mut graph);

        for h in graph.handles() {
            let i = h.id().0;
            println!("node {}: {:?}", i, get_handle(&graph, i as u64));
        }

        let p1_steps = get_steps(&graph, path_1);
        let p2_steps = get_steps(&graph, path_2);

        println!("Path 1: {:?}", p1_steps);
        println!("Path 2: {:?}", p2_steps);

        println!("node 8: {:?}", get_handle(&graph, 8));
        println!("node 4: {:?}", get_handle(&graph, 4));
        println!("node 7: {:?}", get_handle(&graph, 7));
        println!("node 1: {:?}", get_handle(&graph, 1));

        assert_eq!(get_handle(&graph, 8), (vec![], 8, vec![1, 7]));
        assert_eq!(get_handle(&graph, 4), (vec![1, 7], 4, vec![]));
        assert_eq!(get_handle(&graph, 7), (vec![8], 7, vec![4]));
        assert_eq!(get_handle(&graph, 1), (vec![8], 1, vec![4]));

        assert_eq!(p1_steps, vec_hnd(vec![8, 7, 4]));
        assert_eq!(p2_steps, vec_hnd(vec![8, 1, 4]));
    }
}
