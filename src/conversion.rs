use crate::{
    handle::{Edge, Handle, NodeId},
    handlegraph::{HandleGraph, HandleGraphRef},
    mutablehandlegraph::MutableHandleGraph,
    pathgraph::PathHandleGraph,
};

use gfa::{
    gfa::{Line, Link, Orientation, Path, Segment, GFA},
    optfields::OptFields,
    parser::GFAResult,
};

use bstr::BString;

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

pub fn to_gfa<G>(graph: &G) -> GFA<usize, ()>
where
    G: HandleGraphRef + PathHandleGraph,
{
    let mut gfa = GFA::new();

    for handle in graph.all_handles() {
        let name = usize::from(handle.id());
        let sequence: BString = graph.sequence_iter(handle.forward()).collect();

        let segment = Segment {
            name,
            sequence,
            optional: (),
        };
        gfa.segments.push(segment);
    }

    let orient = |rev: bool| {
        if rev {
            Orientation::Backward
        } else {
            Orientation::Forward
        }
    };

    for edge in graph.all_edges() {
        let Edge(left, right) = edge;
        let from_segment: usize = usize::from(left.id());
        let from_orient = orient(left.is_reverse());
        let to_segment: usize = usize::from(right.id());
        let to_orient = orient(right.is_reverse());
        let overlap = BString::from("0M");

        let link = Link {
            from_segment,
            from_orient,
            to_segment,
            to_orient,
            overlap,
            optional: (),
        };

        gfa.links.push(link);
    }

    for path_id in graph.paths_iter() {
        let path_name: BString = graph.path_handle_to_name(path_id).into();
        let overlaps = Vec::new();
        let segment_names: Vec<Vec<u8>> = Vec::new();
        for step in graph.steps_iter(path_id) {
            let handle = graph.handle_of_step(&step).unwrap();
            let segment: usize = handle.id().into();
            let orientation = orient(handle.is_reverse());
            segment_names.push(segment.to_string().into());
            segment_names.push(orientation.to_string().into());
            segment_names.push(",".into());
        }
        let segment_names: BString =
            segment_names.into_iter().flatten().collect();

        let path: Path<usize, ()> = Path {
            path_name,
            segment_names,
            overlaps,
            optional: (),
            _segment_names: std::marker::PhantomData,
        };

        gfa.paths.push(path);
    }

    gfa
}
