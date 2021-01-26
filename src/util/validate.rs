use crate::{
    handle::{Direction, Edge},
    handlegraph::*,
    packedgraph::index::OneBasedIndex,
    pathhandlegraph::*,
};

use crate::packed::*;
use crate::packedgraph::*;

use fnv::FnvHashSet;

#[allow(unused_imports)]
use log::{debug, error, info, trace};

pub fn validate(graph: &PackedGraph) {
    info!("validating graph");

    let mut success = true;

    // all neighbors of all nodes exist && nodes only exist once
    let handles_all = graph.handles().collect::<Vec<_>>();
    let handles_set = graph.handles().collect::<FnvHashSet<_>>();
    let node_count = graph.node_count();

    assert_eq!(handles_all.len(), handles_set.len());
    assert_eq!(handles_set.len(), node_count);

    let handles_len = handles_set.len();
    let mut nodes_set = handles_set
        .into_iter()
        .map(|h| h.id())
        .collect::<FnvHashSet<_>>();
    let nodes_len = nodes_set.len();

    assert_eq!(handles_len, nodes_len);

    for middle in graph.handles() {
        for left in graph.neighbors(middle, Direction::Left) {
            if !graph.has_node(left.id()) {
                nodes_set.insert(left.id());
                info!(
                    "node {}'s left neighbor {} does not exist",
                    middle.id().0,
                    left.id().0
                );
                success = false;
            }
        }
        for right in graph.neighbors(middle, Direction::Right) {
            if !graph.has_node(right.id()) {
                nodes_set.insert(right.id());
                info!(
                    "node {}'s right neighbor {} does not exist",
                    middle.id().0,
                    right.id().0
                );
                success = false;
            }
        }
    }

    let mut edges_count = 0;

    // all edges exist
    for Edge(left, right) in graph.edges() {
        edges_count += 1;
        let left_missing = !graph.has_node(left.id());
        let right_missing = !graph.has_node(right.id());

        if left_missing && right_missing {
            info!(
                "both sides of edge missing:  left {}, right {}",
                left.id().0,
                right.id().0
            );
            success = false;
        } else if left_missing {
            info!("left side of edge missing:   left {}", left.id().0);
            success = false;
            nodes_set.insert(right.id());
        } else if right_missing {
            info!("right side of edge missing: right {}", right.id().0);
            success = false;
            nodes_set.insert(left.id());
        } else {
            nodes_set.insert(right.id());
            nodes_set.insert(left.id());
        }
    }

    assert_eq!(edges_count, graph.edge_count());

    assert_eq!(nodes_len, nodes_set.len());

    // all steps on all paths are on existing nodes
    for path_id in graph.path_ids() {
        let head = graph.path_first_step(path_id).unwrap();
        let tail = graph.path_last_step(path_id).unwrap();

        if head.is_null() && tail.is_null() {
            info!("path with id {} has null head and tail", path_id.0);
        } else if head.is_null() {
            info!("path with id {} has null head", path_id.0);
        } else if tail.is_null() {
            info!("path with id {} has null tail", path_id.0);
        }

        for step in graph.path_steps(path_id).unwrap() {
            let handle = step.handle();
            if !graph.has_node(handle.id()) {
                info!(
                    "path {} step {} is on a nonexistent handle {}",
                    path_id.0,
                    step.0.pack(),
                    handle.0
                );
            } else {
                nodes_set.insert(handle.id());
            }
        }
    }

    assert_eq!(nodes_len, nodes_set.len());

    if success {
        info!("graph successfully validated");
    } else {
        info!("errors when validating graph");
    }
}
