//! Graph overrides from CommunityModules `78b98ae`. Argument validation and
//! demand follow `dive-eval::community`; checked membership precedes indexing.

use dive_ir::StdOp;

use super::{arg, arg_error, enumerable, refuse};
use crate::eval::Machine;
use crate::node::Kind;
use crate::{Error, Handle};

pub(super) fn implements(op: StdOp) -> bool {
    matches!(
        op,
        StdOp::GraphsAreConnectedIn
            | StdOp::GraphsSimplePath
            | StdOp::GraphsIsStronglyConnected
            | StdOp::UndirectedGraphsAreConnectedIn
            | StdOp::UndirectedGraphsSimplePath
            | StdOp::UndirectedGraphsConnectedComponents
    )
}

struct Graph {
    set: Handle,
    nodes: Vec<Handle>,
    edges: Handle,
}

/// `toSetEnum` leaves explicit collections raw; only the later elements demand
/// may normalize them. Non-explicit conversions retain their existing guards.
fn set_conversion(m: &mut Machine, op: StdOp, value: Handle) -> Result<bool, Error> {
    if value.is_compound() && matches!(m.heap.view(value).kind, Kind::Set | Kind::SetEnum) {
        return Ok(true);
    }
    Ok(enumerable(m, op, value)?.is_some())
}

pub(super) fn call(m: &mut Machine, op: StdOp, args: &[Handle]) -> Result<Handle, Error> {
    match op {
        StdOp::GraphsAreConnectedIn | StdOp::UndirectedGraphsAreConnectedIn => {
            connected(m, op, arg(args, 0)?, arg(args, 1)?, arg(args, 2)?)
        }
        StdOp::GraphsSimplePath | StdOp::UndirectedGraphsSimplePath => {
            simple_paths(m, op, arg(args, 0)?)
        }
        StdOp::GraphsIsStronglyConnected => strongly_connected(m, op, arg(args, 0)?),
        StdOp::UndirectedGraphsConnectedComponents => components(m, op, arg(args, 0)?),
        _ => Err(Error::internal("not a graph operator")),
    }
}

fn graph(m: &mut Machine, op: StdOp, position: &str, value: Handle) -> Result<Graph, Error> {
    let kind = if op.module() == "Graphs" {
        "graph"
    } else {
        "undirected graph"
    };
    let bad = |detail: &str| arg_error(position, op.name(), &format!("{kind} {detail}"));
    if !value.is_compound() {
        return Err(bad("record with a node and an edge field"));
    }
    let record = m.heap.view(value);
    if record.kind == Kind::Lambda {
        return Err(refuse(
            op,
            "a lazy graph record whose conversion is not proved",
        ));
    }
    if (record.kind != Kind::Fcn || record.keys().iter().any(|key| key.as_str_id().is_none()))
        && (!record.kind.is_fcn() || !record.data.is_empty())
    {
        return Err(bad("record with a node and an edge field"));
    }
    let field = |name: &str| {
        record
            .keys()
            .iter()
            .zip(record.values())
            .find_map(|(&key, &v)| {
                key.as_str_id()
                    .and_then(|id| m.heap.store().strings().text(id))
                    .filter(|text| text.as_ref() == name)
                    .map(|_| v)
            })
    };
    let (nodes, edges) = (field("node"), field("edge"));
    let Some(nodes) = nodes else {
        return Err(bad("record whose node field is a set"));
    };
    if !set_conversion(m, op, nodes)? {
        return Err(bad("record whose node field is a set"));
    }
    let Some(edges) = edges else {
        // The argument diagnostic prints the graph. Preserve the existing
        // checked node normalization/refusal on that actual error path.
        enumerable(m, op, nodes)?;
        return Err(bad("record whose edge field is a set"));
    };
    if !set_conversion(m, op, edges)? {
        enumerable(m, op, nodes)?;
        return Err(bad("record whose edge field is a set"));
    }
    let mut nodes = enumerable(m, op, nodes)?
        .ok_or_else(|| Error::internal("a validated graph node set changed kind"))?;
    let set = m.heap.set_from(&mut nodes);
    Ok(Graph { set, nodes, edges })
}

fn endpoints(m: &mut Machine, op: StdOp, edge: Handle) -> Result<Option<(Handle, Handle)>, Error> {
    if op.module() != "Graphs" {
        return Ok(match enumerable(m, op, edge)? {
            Some(nodes) if nodes.len() == 1 => Some((nodes[0], nodes[0])),
            Some(nodes) if nodes.len() == 2 => Some((nodes[0], nodes[1])),
            _ => None,
        });
    }
    if !edge.is_compound() {
        return Ok(None);
    }
    let edge = m.heap.view(edge);
    Ok(
        (edge.kind == Kind::IntFcn && edge.lo == 1 && edge.data.len() == 2)
            .then(|| (edge.data[0], edge.data[1])),
    )
}

/// Membership establishes the comparison contract before extensional lookup.
fn node_index(m: &mut Machine, graph: &Graph, node: Handle) -> Result<Option<usize>, Error> {
    m.charge(1)?;
    if !m.member(node, graph.set)? {
        return Ok(None);
    }
    graph
        .nodes
        .binary_search_by(|&value| m.heap.cmp(value, node))
        .map(Some)
        .map_err(|_| Error::internal("graph member has no extensional node index"))
}

fn adjacency(
    m: &mut Machine,
    op: StdOp,
    graph: &Graph,
    transpose: bool,
) -> Result<Vec<Vec<usize>>, Error> {
    m.charge(graph.nodes.len())?;
    let mut arcs = vec![Vec::new(); graph.nodes.len()];
    let edges = enumerable(m, op, graph.edges)?
        .ok_or_else(|| Error::internal("a validated graph edge set changed kind"))?;
    for edge in edges {
        m.charge(1)?;
        let Some((from, to)) = endpoints(m, op, edge)? else {
            continue;
        };
        let Some(from) = node_index(m, graph, from)? else {
            continue;
        };
        let Some(to) = node_index(m, graph, to)? else {
            continue;
        };
        let (from, to) = if transpose { (to, from) } else { (from, to) };
        arcs[from].push(to);
        if op.module() != "Graphs" && from != to {
            arcs[to].push(from);
        }
    }
    Ok(arcs)
}

fn reachable(m: &mut Machine, arcs: &[Vec<usize>], source: usize) -> Result<Vec<bool>, Error> {
    m.charge(arcs.len())?;
    let mut seen = vec![false; arcs.len()];
    let mut pending = vec![source];
    seen[source] = true;
    while let Some(node) = pending.pop() {
        for &next in &arcs[node] {
            m.charge(1)?;
            if !seen[next] {
                seen[next] = true;
                pending.push(next);
            }
        }
    }
    Ok(seen)
}

fn connected(
    m: &mut Machine,
    op: StdOp,
    from: Handle,
    to: Handle,
    value: Handle,
) -> Result<Handle, Error> {
    let graph = graph(m, op, "third", value)?;
    let Some(from_index) = node_index(m, &graph, from)? else {
        return Ok(Handle::FALSE);
    };
    let Some(to_index) = node_index(m, &graph, to)? else {
        return Ok(Handle::FALSE);
    };
    if crate::compare::equals(&m.heap, from, to)? {
        return Ok(Handle::TRUE);
    }
    let arcs = adjacency(m, op, &graph, false)?;
    Ok(if reachable(m, &arcs, from_index)?[to_index] {
        Handle::TRUE
    } else {
        Handle::FALSE
    })
}

fn strongly_connected(m: &mut Machine, op: StdOp, value: Handle) -> Result<Handle, Error> {
    let graph = graph(m, op, "first", value)?;
    if graph.nodes.is_empty() {
        return Ok(Handle::TRUE);
    }
    let arcs = adjacency(m, op, &graph, false)?;
    if reachable(m, &arcs, 0)?.contains(&false) {
        return Ok(Handle::FALSE);
    }
    let arcs = adjacency(m, op, &graph, true)?;
    Ok(if reachable(m, &arcs, 0)?.contains(&false) {
        Handle::FALSE
    } else {
        Handle::TRUE
    })
}

fn components(m: &mut Machine, op: StdOp, value: Handle) -> Result<Handle, Error> {
    let graph = graph(m, op, "first", value)?;
    let arcs = adjacency(m, op, &graph, false)?;
    let mut seen = vec![false; graph.nodes.len()];
    let mut result = Vec::new();
    for root in 0..graph.nodes.len() {
        m.charge(1)?;
        if seen[root] {
            continue;
        }
        let mut pending = vec![root];
        let mut component = Vec::new();
        seen[root] = true;
        while let Some(node) = pending.pop() {
            component.push(graph.nodes[node]);
            for &next in &arcs[node] {
                m.charge(1)?;
                if !seen[next] {
                    seen[next] = true;
                    pending.push(next);
                }
            }
        }
        result.push(m.heap.set_from(&mut component));
    }
    Ok(m.heap.set_from(&mut result))
}

fn simple_paths(m: &mut Machine, op: StdOp, value: Handle) -> Result<Handle, Error> {
    let graph = graph(m, op, "first", value)?;
    let arcs = adjacency(m, op, &graph, false)?;
    let mut seen = vec![false; graph.nodes.len()];
    let mut path = Vec::new();
    let mut stack = Vec::new();
    let mut result = Vec::new();
    for root in 0..graph.nodes.len() {
        seen[root] = true;
        path.push(graph.nodes[root]);
        stack.push((root, 0));
        m.charge(1)?;
        result.push(m.heap.tuple(&path));
        while let Some((node, offset)) = stack.last_mut() {
            m.charge(1)?;
            if let Some(&next) = arcs[*node].get(*offset) {
                *offset += 1;
                if !seen[next] {
                    seen[next] = true;
                    path.push(graph.nodes[next]);
                    result.push(m.heap.tuple(&path));
                    stack.push((next, 0));
                }
            } else {
                seen[*node] = false;
                stack.pop();
                path.pop();
            }
        }
    }
    // Keep TLC's unnormalized DFS-prefix order until a caller demands it.
    Ok(m.heap.set_enum(&result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memo::Memo;
    use crate::{Opts, Store};
    use std::sync::Arc;

    fn machine() -> Machine {
        Machine::new(
            Arc::new(Store::default()),
            0,
            Opts::default(),
            Memo::new(&[]),
        )
    }

    fn record(m: &mut Machine, nodes: Handle, edges: Handle) -> Result<Handle, Error> {
        let node_key = m.heap.string("node")?;
        let edge_key = m.heap.string("edge")?;
        Ok(m.heap
            .fcn_from_pairs(&mut [(node_key, nodes), (edge_key, edges)]))
    }

    #[test]
    fn distinct_compound_handles_are_extensional_nodes() -> Result<(), Error> {
        let mut m = machine();
        let one = m.heap.int_from_i64(1);
        let two = m.heap.int_from_i64(2);
        let first = m.heap.tuple(&[one]);
        let second = m.heap.tuple(&[two]);
        let nodes = m.heap.set_from(&mut vec![first, second]);
        let same_first = m.heap.tuple(&[one]);
        let same_second = m.heap.tuple(&[two]);
        assert_ne!(first, same_first);
        let edge = m.heap.tuple(&[same_first, same_second]);
        let back = m.heap.tuple(&[same_second, same_first]);
        let edges = m.heap.set_from(&mut vec![edge, back]);
        let graph = record(&mut m, nodes, edges)?;
        assert_eq!(
            strongly_connected(&mut m, StdOp::GraphsIsStronglyConnected, graph)?,
            Handle::TRUE
        );
        assert_eq!(
            connected(
                &mut m,
                StdOp::GraphsAreConnectedIn,
                same_first,
                same_second,
                graph
            )?,
            Handle::TRUE
        );
        let paths = simple_paths(&mut m, StdOp::GraphsSimplePath, graph)?;
        assert_eq!(m.heap.cardinality(paths)?, 4);
        Ok(())
    }

    #[test]
    fn graph_validation_and_work_limits_never_certify_a_result() -> Result<(), Error> {
        let mut m = machine();
        let one = m.heap.int_from_i64(1);
        let op = StdOp::UndirectedGraphsConnectedComponents;
        let error = components(&mut m, op, one);
        assert!(
            matches!(error, Err(ref e) if e.tlc() == Some(crate::TlcClass::eval_exception(2169)))
        );
        let empty = m.heap.empty_set();
        let malformed = record(&mut m, one, empty)?;
        let error = components(&mut m, op, malformed);
        assert!(
            matches!(error, Err(ref e) if e.tlc() == Some(crate::TlcClass::eval_exception(2169)))
        );
        let nodes = m.heap.set_from(&mut vec![one]);
        let graph = record(&mut m, nodes, empty)?;
        m.set_fuel(0);
        assert!(components(&mut m, op, graph).is_err());
        Ok(())
    }
}
