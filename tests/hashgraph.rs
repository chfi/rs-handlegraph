use handlegraph::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::HandleGraph,
    hashgraph::{HashGraph, PathStep},
    mutablehandlegraph::MutableHandleGraph,
    pathgraph::PathHandleGraph,
};

static H1: Handle = Handle::from_integer(2);
static H2: Handle = Handle::from_integer(4);
static H3: Handle = Handle::from_integer(6);
static H4: Handle = Handle::from_integer(8);
static H5: Handle = Handle::from_integer(10);
static H6: Handle = Handle::from_integer(12);

#[test]
fn can_create_handles() {
    let mut graph = HashGraph::new();
    let h1 = graph.append_handle(b"CAAATAAG");
    let h2 = graph.append_handle(b"A");
    let h3 = graph.append_handle(b"G");

    let n1 = graph.get_node_unsafe(&h1.id());
    let n2 = graph.get_node_unsafe(&h2.id());
    let n3 = graph.get_node_unsafe(&h3.id());

    assert_eq!(u64::from(h1), 1);
    assert_eq!(u64::from(h3), 3);

    assert_eq!(n1.sequence.as_slice(), b"CAAATAAG");
    assert_eq!(n2.sequence.as_slice(), b"A");
    assert_eq!(n3.sequence.as_slice(), b"G");
}

#[test]
fn can_create_edges() {
    let mut graph = HashGraph::new();
    let h1 = graph.append_handle(b"CAAATAAG");
    let h2 = graph.append_handle(b"A");
    let h3 = graph.append_handle(b"G");
    let h4 = graph.append_handle(b"TTG");

    graph.create_edge(&Edge(h1, h2));
    graph.create_edge(&Edge(h1, h3));
    graph.create_edge(&Edge(h2, h4));
    graph.create_edge(&Edge(h3, h4));

    let n1 = graph.get_node_unsafe(&h1.id());
    let n2 = graph.get_node_unsafe(&h2.id());
    let n3 = graph.get_node_unsafe(&h3.id());
    let n4 = graph.get_node_unsafe(&h4.id());

    assert_eq!(true, n1.right_edges.contains(&h2));
    assert_eq!(true, n1.right_edges.contains(&h3));

    assert_eq!(true, n2.left_edges.contains(&h1.flip()));
    assert_eq!(true, n2.right_edges.contains(&h4));
    assert_eq!(true, n3.left_edges.contains(&h1.flip()));
    assert_eq!(true, n3.right_edges.contains(&h4));

    assert_eq!(true, n4.left_edges.contains(&h2.flip()));
    assert_eq!(true, n4.left_edges.contains(&h3.flip()));
}

fn read_test_gfa() -> HashGraph {
    use gfa::gfa::GFA;
    use gfa::parser::GFAParser;

    let parser = GFAParser::new();
    let gfa: GFA<usize, ()> = parser.parse_file("./lil.gfa").unwrap();

    HashGraph::from_gfa(&gfa)
}

#[test]
fn construct_from_gfa() {
    use gfa::gfa::GFA;
    use gfa::parser::GFAParser;

    let parser = GFAParser::new();
    let gfa: Option<GFA<usize, ()>> = parser.parse_file("./lil.gfa").ok();

    if let Some(gfa) = gfa {
        let graph = HashGraph::from_gfa(&gfa);
        let node_ids: Vec<_> = graph.graph.keys().collect();

        assert_eq!(15, graph.node_count());
        assert_eq!(40, graph.edge_count());
        println!("Node IDs:");
        for id in node_ids.iter() {
            println!("{:?}", id);
            let node = graph.graph.get(id).unwrap();
            println!("{:?}", Handle::pack(**id, false));
            let lefts: Vec<_> = node.left_edges.iter().collect();
            println!("lefts: {:?}", lefts);
            let rights: Vec<_> = node.right_edges.iter().collect();
            println!("rights: {:?}", rights);
            println!("{:?}", graph.graph.get(id));
        }
    } else {
        panic!("Couldn't parse test GFA file!");
    }
}

#[test]
fn degree_is_correct() {
    let graph = read_test_gfa();

    let h1 = Handle::pack(NodeId::from(9 as u64), false);
    let h2 = Handle::pack(NodeId::from(3 as u64), false);

    assert_eq!(graph.degree(h1, Direction::Right), 2);
    assert_eq!(graph.degree(h1, Direction::Left), 2);
    assert_eq!(graph.degree(h2, Direction::Right), 2);
    assert_eq!(graph.degree(h2, Direction::Left), 1);
}

fn path_graph() -> HashGraph {
    let mut graph = HashGraph::new();
    let h1 = graph.create_handle(b"1", NodeId::from(1 as u64));
    let h2 = graph.create_handle(b"2", NodeId::from(2 as u64));
    let h3 = graph.create_handle(b"3", NodeId::from(3 as u64));
    let h4 = graph.create_handle(b"4", NodeId::from(4 as u64));
    let h5 = graph.create_handle(b"5", NodeId::from(5 as u64));
    let h6 = graph.create_handle(b"6", NodeId::from(6 as u64));

    /*
    edges
    1  -> 2 -> 5 -> 6
      \-> 3 -> 4 /
     */
    graph.create_edge(&Edge(h1, h2));
    graph.create_edge(&Edge(h2, h5));
    graph.create_edge(&Edge(h5, h6));

    graph.create_edge(&Edge(h1, h3));
    graph.create_edge(&Edge(h3, h4));
    graph.create_edge(&Edge(h4, h6));

    graph
}

#[test]
fn graph_has_edge() {
    let graph: HashGraph = read_test_gfa();

    let h18 = Handle::from_integer(18);
    let h19 = h18.flip();
    let h20 = Handle::from_integer(20);
    let h21 = h20.flip();

    assert!(graph.has_edge(h18, h20));
    assert!(graph.has_edge(h21, h19));
}

#[test]
fn graph_handle_edges_iter() {
    let mut graph = path_graph();

    graph.create_edge(&Edge(H1, H4));
    graph.create_edge(&Edge(H1, H6));

    let mut iter = graph.handle_edges_iter(H1, Direction::Right);

    assert_eq!(Some(H2), iter.next());
    assert_eq!(Some(H3), iter.next());
    assert_eq!(Some(H4), iter.next());
    assert_eq!(Some(H6), iter.next());
    assert_eq!(None, iter.next());
}

#[test]
fn graph_handles_iter() {
    let graph = path_graph();

    let iter = graph.handles_iter();

    let nodes: Vec<_> = vec![H1, H2, H3, H4, H5, H6]
        .into_iter()
        .map(|x| x.id())
        .collect();

    let mut iter_nodes: Vec<NodeId> = vec![];

    for h in iter {
        iter_nodes.push(h.id())
    }

    assert!(iter_nodes.iter().all(|n| graph.get_node(n).is_some()));
    assert!(nodes.iter().all(|n| iter_nodes.contains(n)));
}

#[test]
fn graph_edges_iter() {
    let mut graph = path_graph();

    graph.create_edge(&Edge(H1, H4));
    graph.create_edge(&Edge(H1, H6));

    graph.create_edge(&Edge(H4, H2));
    graph.create_edge(&Edge(H6, H2));

    graph.create_edge(&Edge(H3, H5));

    let mut edges_found: Vec<_> = graph.edges_iter().collect();

    edges_found.sort();

    let mut edges: Vec<_> = vec![
        Edge::edge_handle(H1, H2),
        Edge::edge_handle(H1, H3),
        Edge::edge_handle(H1, H4),
        Edge::edge_handle(H1, H6),
        Edge::edge_handle(H2, H5),
        Edge::edge_handle(H4, H2),
        Edge::edge_handle(H6, H2),
        Edge::edge_handle(H3, H4),
        Edge::edge_handle(H3, H5),
        Edge::edge_handle(H4, H6),
        Edge::edge_handle(H5, H6),
    ];

    edges.sort();

    assert_eq!(edges, edges_found);
}

#[test]
fn append_prepend_path() {
    let mut graph = path_graph();

    // Add a path 3 -> 5

    let p1 = graph.create_path_handle(b"path-1", false);
    graph.append_step(&p1, H3);
    graph.append_step(&p1, H5);

    // Add another path 1 -> 3 -> 4 -> 6

    let p2 = graph.create_path_handle(b"path-2", false);
    graph.append_step(&p2, H1);
    let p2_3 = graph.append_step(&p2, H3);
    let p2_4 = graph.append_step(&p2, H4);
    graph.append_step(&p2, H6);

    let test_node = |graph: &HashGraph,
                     nid: u64,
                     o1: Option<&usize>,
                     o2: Option<&usize>| {
        let n = graph.get_node(&NodeId::from(nid)).unwrap();
        assert_eq!(o1, n.occurrences.get(&p1));
        assert_eq!(o2, n.occurrences.get(&p2));
    };

    // At this point, node 3 should have two occurrences entries,
    // index 0 for path 1, index 1 for path 2
    test_node(&graph, 3, Some(&0), Some(&1));

    // Node 1 should have only one occurrence at the start of path 2
    test_node(&graph, 1, None, Some(&0));

    // Node 6 should have only one occurrence at the end of path 2
    test_node(&graph, 6, None, Some(&3));

    // Now, append node 6 to path 1

    graph.append_step(&p1, H6);

    // Node 6 should also occur at the end of path 1
    test_node(&graph, 6, Some(&2), Some(&3));

    // The other nodes should be unaffected
    test_node(&graph, 1, None, Some(&0));
    test_node(&graph, 4, None, Some(&2));

    test_node(&graph, 3, Some(&0), Some(&1));
    test_node(&graph, 5, Some(&1), None);

    // Now, prepend node 1 to path 1
    graph.prepend_step(&p1, H1);

    // Node 1 should be the first in both paths
    test_node(&graph, 1, Some(&0), Some(&0));

    // The other nodes should have had 1 added to their
    // occurrences in path 1, while the path 2 ones should be the
    // same
    test_node(&graph, 3, Some(&1), Some(&1));
    test_node(&graph, 5, Some(&2), None);
    test_node(&graph, 6, Some(&3), Some(&3));

    test_node(&graph, 4, None, Some(&2));

    // At this point path 1 is 1 -> 3 -> 5 -> 6, path 2 is unmodified
    // Rewrite the segment 3 -> 4 in path 2 with the empty path
    graph.rewrite_segment(&p2_3, &p2_4, vec![]);

    // Node 1 should be the same
    test_node(&graph, 1, Some(&0), Some(&0));

    // Node 6 should have been decremented by 2 in path 2
    test_node(&graph, 6, Some(&3), Some(&1));

    // Nodes 3, 4 should be empty in path 2
    test_node(&graph, 3, Some(&1), None);
    test_node(&graph, 4, None, None);

    // Rewrite the segment 1 -> 6 in path 2 with the segment
    // 6 -> 4 -> 5 -> 3 -> 1 -> 2
    graph.rewrite_segment(
        &PathStep::Step(1, 0),
        &PathStep::Step(1, 1),
        vec![H6, H4, H5, H3, H1, H2],
    );

    // The path 2 occurrences should be correctly updated for all nodes
    test_node(&graph, 1, Some(&0), Some(&4));
    test_node(&graph, 2, None, Some(&5));
    test_node(&graph, 3, Some(&1), Some(&3));
    test_node(&graph, 4, None, Some(&1));
    test_node(&graph, 5, Some(&2), Some(&2));
    test_node(&graph, 6, Some(&3), Some(&0));

    // Rewrite the segment Front(_) .. 5 in path 1 with the segment [2, 3]
    graph.rewrite_segment(
        &PathStep::Front(0),
        &PathStep::Step(0, 2),
        vec![H2, H3],
    );

    // Now path 1 is 2 -> 3 -> 6
    test_node(&graph, 1, None, Some(&4));
    test_node(&graph, 2, Some(&0), Some(&5));
    test_node(&graph, 3, Some(&1), Some(&3));
    test_node(&graph, 5, None, Some(&2));
    test_node(&graph, 6, Some(&2), Some(&0));

    // Rewrite the segment 3 .. End(_) in path 2 with the segment [1]
    graph.rewrite_segment(&PathStep::Step(1, 3), &PathStep::End(1), vec![H1]);

    // Now path 2 is 6 -> 4 -> 5 -> 1
    test_node(&graph, 1, None, Some(&3));
    test_node(&graph, 2, Some(&0), None);
    test_node(&graph, 3, Some(&1), None);
    test_node(&graph, 4, None, Some(&1));
    test_node(&graph, 5, None, Some(&2));
    test_node(&graph, 6, Some(&2), Some(&0));

    graph.print_path(&p1);
    graph.print_path(&p2);

    graph.print_occurrences();
}

#[test]
fn graph_path_steps_iter() {
    use handlegraph::hashgraph::PathStep::*;

    let mut graph = path_graph();

    let p1 = graph.create_path_handle(b"path-1", false);
    graph.append_step(&p1, H1);
    graph.append_step(&p1, H2);
    graph.append_step(&p1, H5);
    graph.append_step(&p1, H6);

    let mut iter = graph.steps_iter(&p1);

    assert_eq!(Some(Step(p1, 0)), iter.next());
    assert_eq!(Some(Step(p1, 1)), iter.next());
    assert_eq!(Some(Step(p1, 2)), iter.next());
    assert_eq!(Some(Step(p1, 3)), iter.next());
    assert_eq!(None, iter.next());
}

#[test]
fn graph_divide_handle() {
    let mut graph = HashGraph::new();
    graph.append_handle(b"ABCD");
    graph.append_handle(b"EFGHIJKLMN");
    graph.append_handle(b"OPQ");

    graph.create_edge(&Edge(H1, H2));
    graph.create_edge(&Edge(H2, H3));

    let path = graph.create_path_handle(b"path-1", false);

    let walk_path = |graph: &HashGraph| {
        let mut last = graph.path_front_end(&path);
        let mut handles = vec![];
        for _ in 0..graph.step_count(&path) {
            let next = graph.next_step(&last);
            handles.push(graph.handle_of_step(&next));
            last = next;
        }
        handles
    };

    graph.append_step(&path, H1);
    graph.append_step(&path, H2);
    graph.append_step(&path, H3);

    assert_eq!(b"ABCD", graph.sequence(H1));
    assert_eq!(b"EFGHIJKLMN", graph.sequence(H2));
    assert_eq!(b"OPQ", graph.sequence(H3));

    assert!(graph.has_edge(H1, H2));
    assert!(graph.has_edge(H2, H3));

    let handles = walk_path(&graph);

    let expected_handles: Vec<_> =
        [H1, H2, H3].iter().map(|h| Some(*h)).collect();

    assert_eq!(expected_handles, handles);

    graph.divide_handle(H2, vec![3, 7, 9]);

    // The left-hand edges on the divided handle are the same
    assert!(graph.has_edge(H1, H2));
    // But the right-hand are not
    assert!(!graph.has_edge(H2, H3));

    // The new handles are chained together
    assert!(graph.has_edge(H2, H4));
    assert!(graph.has_edge(H4, H5));
    assert!(graph.has_edge(H5, H6));
    // and the last one attaches to the correct node on its RHS
    assert!(graph.has_edge(H6, H3));

    // The other handles are untouched
    assert_eq!(graph.sequence(H1), b"ABCD");
    assert_eq!(graph.sequence(H3), b"OPQ");

    // The split handle has a corresponding subsequence
    assert_eq!(graph.sequence(H2), b"EFG");

    // The new handles are correctly constructed
    assert_eq!(graph.sequence(H4), b"HIJK");
    assert_eq!(graph.sequence(H5), b"LM");
    assert_eq!(graph.sequence(H6), b"N");

    // The path is correctly updated
    let handles = walk_path(&graph);

    let expected_handles: Vec<_> =
        [H1, H2, H4, H5, H6, H3].iter().map(|h| Some(*h)).collect();

    assert_eq!(expected_handles, handles);
}
