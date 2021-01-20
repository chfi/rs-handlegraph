use crate::{
    handle::{Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::*,
    pathhandlegraph::*,
};

use crate::packedgraph::PackedGraph;

use gfa::{
    gfa::{Line, Link, Orientation, Path, Segment, GFA},
    optfields::OptFields,
    parser::GFAResult,
};

pub fn from_gfa<G, T>(gfa: &GFA<usize, T>) -> G
where
    G: Default + AdditiveHandleGraph + MutableGraphPaths,
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
        let path_id = graph.create_path(name, false).unwrap();
        for (seg, orient) in path.iter() {
            let handle = Handle::new(seg, orient);
            graph.path_append_step(path_id, handle);
        }
    }

    graph
}

pub fn fill_gfa_lines<G, I, T>(graph: &mut G, gfa_lines: I) -> GFAResult<()>
where
    G: AdditiveHandleGraph + MutableGraphPaths,
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
                let path_id = graph.create_path(name, false).unwrap();
                for (seg, orient) in v.iter() {
                    let handle = Handle::new(seg, orient);
                    graph.path_append_step(path_id, handle);
                }
            }
            _ => (),
        }
    }

    Ok(())
}

pub fn packed_to_gfa(graph: &PackedGraph) -> GFA<usize, ()> {
    to_gfa(graph)
}

pub fn to_gfa<G>(graph: G) -> GFA<usize, ()>
where
    G: HandleGraphRef + GraphPathsSteps + IntoPathIds + GraphPathNames,
{
    let mut gfa = GFA::new();

    for handle in graph.handles() {
        let name = usize::from(handle.id());
        let sequence: Vec<_> = graph.sequence(handle.forward()).collect();

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

    for Edge(left, right) in graph.edges() {
        let from_segment = usize::from(left.id());
        let from_orient = orient(left.is_reverse());

        let to_segment = usize::from(right.id());
        let to_orient = orient(right.is_reverse());
        let overlap = vec![b'0', b'M'];

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

    for path_id in graph.path_ids() {
        let path_name: Vec<_> = graph.get_path_name(path_id).unwrap().collect();
        let overlaps = Vec::new();
        let mut segment_names: Vec<Vec<u8>> = Vec::new();

        let steps = graph.path_steps(path_id).unwrap();

        for step in steps {
            let handle = step.handle();
            let segment: usize = handle.id().into();
            let orientation = orient(handle.is_reverse());
            segment_names.push(segment.to_string().into());
            segment_names.push(orientation.to_string().into());
            segment_names.push(",".into());
        }
        let segment_names: Vec<u8> =
            segment_names.into_iter().flatten().collect();

        let path: Path<usize, ()> =
            Path::new(path_name, segment_names, overlaps, ());

        gfa.paths.push(path);
    }

    gfa
}

pub fn write_as_gfa<O: std::io::Write, G>(
    graph: G,
    stream: &mut O,
) -> std::io::Result<()>
where
    G: HandleGraphRef + GraphPathsSteps + IntoPathIds + GraphPathNames,
{
    use bstr::ByteSlice;

    // header
    writeln!(stream, "H\tVN:Z:1.0")?;

    for handle in graph.handles() {
        let name = handle.id().0;
        let sequence = graph.sequence_vec(handle.forward());
        writeln!(stream, "S\t{}\t{}", name, sequence.as_bstr())?;
    }

    let fmt_orient = |rev: bool| {
        if rev {
            "-"
        } else {
            "+"
        }
    };

    for Edge(left, right) in graph.edges() {
        let from_segment = left.id().0;
        let from_orient = fmt_orient(left.is_reverse());

        let to_segment = right.id().0;
        let to_orient = fmt_orient(right.is_reverse());

        writeln!(
            stream,
            "L\t{}\t{}\t{}\t{}\t0M",
            from_segment, from_orient, to_segment, to_orient
        )?;
    }

    for path_id in graph.path_ids() {
        let path_name: Vec<_> = graph.get_path_name(path_id).unwrap().collect();
        write!(stream, "P\t{}\t", path_name.as_bstr())?;

        let steps = graph.path_steps(path_id).unwrap();
        for (ix, step) in steps.enumerate() {
            if ix != 0 {
                write!(stream, ",")?;
            }
            let handle = step.handle();

            let seg = handle.id().0;
            let orient = fmt_orient(handle.is_reverse());

            write!(stream, "{}{}", seg, orient)?;
        }

        writeln!(stream, "\t*")?;
    }

    Ok(())
}
