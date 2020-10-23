use crate::{
    handle::{Edge, Handle, NodeId},
    handlegraph::HandleGraph,
    mutablehandlegraph::MutableHandleGraph,
    pathgraph::PathHandleGraph,
};

use gfa::{
    gfa::{Line, GFA},
    optfields::OptFields,
    parser::GFAResult,
};

pub fn from_gfa<G, T>(gfa: &GFA<usize, T>) -> G
where
    G: Default + MutableHandleGraph + PathHandleGraph,
    T: OptFields,
{
    let mut graph: G = Default::default();

    for segment in gfa.segments.iter() {
        assert!(segment.name > 0);
        let seq = &segment.sequence;
        graph.create_handle(seq, segment.name);
    }

    for link in gfa.links.iter() {
        let left = Handle::new(link.from_segment, link.from_orient);
        let right = Handle::new(link.from_segment, link.from_orient);
        graph.create_edge(Edge(left, right));
    }

    for path in gfa.paths.iter() {
        let name = &path.path_name;
        let path_id = graph.create_path_handle(name, false);
        for (seg, orient) in path.iter() {
            let handle = Handle::new(seg, orient);
            graph.append_step(&path_id, handle);
        }
    }

    graph
}

pub fn fill_gfa_lines<G, I, T>(graph: &mut G, gfa_lines: I) -> GFAResult<()>
where
    G: MutableHandleGraph + PathHandleGraph,
    I: Iterator<Item = GFAResult<Line<usize, T>>>,
    T: OptFields,
{
    for line in gfa_lines {
        let line = line?;
        match line {
            Line::Segment(v) => {
                let id = NodeId::from(v.name);
                graph.create_handle(&v.sequence, id);
            }
            Line::Link(v) => {
                let left = Handle::new(v.from_segment, v.from_orient);
                let right = Handle::new(v.to_segment, v.to_orient);
                graph.create_edge(Edge(left, right));
            }
            Line::Path(v) => {
                let name = &v.path_name;
                let path_id = graph.create_path_handle(name, false);
                for (seg, orient) in v.iter() {
                    let handle = Handle::new(seg, orient);
                    graph.append_step(&path_id, handle);
                }
            }
            _ => (),
        }
    }

    Ok(())
}
