use crate::error::DagError;
use std::collections::{HashMap, VecDeque};
use types::{Edge, Node};
use uuid::Uuid;

pub fn topological_sort(nodes: &[Node], edges: &[Edge]) -> Result<Vec<Uuid>, DagError> {
    let mut in_degree: HashMap<_, usize> = nodes.iter().map(|n| (n.id, 0)).collect();
    let mut adjacency: HashMap<_, Vec<_>> = nodes.iter().map(|n| (n.id, vec![])).collect();

    for edge in edges {
        adjacency
            .get_mut(&edge.source_node)
            .unwrap()
            .push(edge.target_node);
        *in_degree.get_mut(&edge.target_node).unwrap() += 1;
    }

    let mut queue: VecDeque<_> = in_degree
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(id, _)| *id)
        .collect();

    let mut result = Vec::new();

    while let Some(node_id) = queue.pop_front() {
        result.push(node_id);
        for &neighbor in adjacency.get(&node_id).unwrap() {
            *in_degree.get_mut(&neighbor).unwrap() -= 1;
            if in_degree[&neighbor] == 0 {
                queue.push_back(neighbor);
            }
        }
    }

    if result.len() != nodes.len() {
        return Err(DagError::CycleDetected);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{Edge, Node};
    use uuid::Uuid;

    #[test]
    fn test_topological_sort() {
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
        let node3 = Node::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "c".to_string(),
            "".to_string(),
        );
        let nodes = vec![node1.clone(), node2.clone(), node3.clone()];

        let edge1 = Edge::new(nodes[0].run_id, nodes[0].id, nodes[1].id);
        let edge2 = Edge::new(nodes[0].run_id, nodes[1].id, nodes[2].id);
        let edges = vec![edge1, edge2];

        let sorted = topological_sort(&nodes, &edges).unwrap();
        assert_eq!(sorted.len(), 3);
        assert!(sorted[0] == nodes[0].id);
        assert!(sorted[2] == nodes[2].id);
    }
}
