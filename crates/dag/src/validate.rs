use crate::error::DagError;
use std::collections::{HashMap, HashSet};
use types::{Edge, Node};

pub fn validate_dag(nodes: &[Node], edges: &[Edge]) -> Result<(), DagError> {
    if nodes.is_empty() {
        return Err(DagError::EmptyNodeList);
    }

    let node_ids: HashSet<_> = nodes.iter().map(|n| n.id).collect();

    let mut in_degree: HashMap<_, usize> = nodes.iter().map(|n| (n.id, 0)).collect();
    let mut adjacency: HashMap<_, Vec<_>> = nodes.iter().map(|n| (n.id, vec![])).collect();

    for edge in edges {
        if !node_ids.contains(&edge.source_node) {
            return Err(DagError::InvalidEdgeReference(format!(
                "source node {} not found",
                edge.source_node
            )));
        }
        if !node_ids.contains(&edge.target_node) {
            return Err(DagError::InvalidEdgeReference(format!(
                "target node {} not found",
                edge.target_node
            )));
        }

        adjacency
            .get_mut(&edge.source_node)
            .unwrap()
            .push(edge.target_node);
        *in_degree.get_mut(&edge.target_node).unwrap() += 1;
    }

    let mut queue: Vec<_> = in_degree
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut visited = 0;

    while let Some(node_id) = queue.pop() {
        visited += 1;
        for &neighbor in adjacency.get(&node_id).unwrap() {
            *in_degree.get_mut(&neighbor).unwrap() -= 1;
            if in_degree[&neighbor] == 0 {
                queue.push(neighbor);
            }
        }
    }

    if visited != nodes.len() {
        return Err(DagError::CycleDetected);
    }

    let root_ids: HashSet<_> = edges.iter().map(|e| e.target_node).collect();
    let has_roots = nodes.iter().any(|n| !root_ids.contains(&n.id));
    if !has_roots && !nodes.is_empty() {
        return Err(DagError::OrphanNode("no root nodes found".to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{Edge, Node};
    use uuid::Uuid;

    #[test]
    fn test_valid_dag() {
        let node1 = Node::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "a".to_string(),
            "".to_string(),
        );
        let node2 = Node::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "b".to_string(),
            "".to_string(),
        );
        let nodes = vec![node1.clone(), node2.clone()];

        let edge = Edge::new(nodes[0].run_id, nodes[0].id, nodes[1].id);
        let edges = vec![edge];

        validate_dag(&nodes, &edges).unwrap();
    }

    #[test]
    fn test_cycle_detected() {
        let node1 = Node::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "a".to_string(),
            "".to_string(),
        );
        let node2 = Node::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "b".to_string(),
            "".to_string(),
        );
        let nodes = vec![node1.clone(), node2.clone()];

        let edge1 = Edge::new(nodes[0].run_id, nodes[0].id, nodes[1].id);
        let edge2 = Edge::new(nodes[0].run_id, nodes[1].id, nodes[0].id);
        let edges = vec![edge1, edge2];

        let result = validate_dag(&nodes, &edges);
        assert!(result.is_err());
    }
}
