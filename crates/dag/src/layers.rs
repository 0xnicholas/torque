use crate::error::DagError;
use std::collections::HashMap;
use types::{Edge, Node};
use uuid::Uuid;

pub type Layer = i32;

pub fn compute_layers(nodes: &[Node], edges: &[Edge]) -> Result<HashMap<Uuid, Layer>, DagError> {
    let mut in_degree: HashMap<_, usize> = nodes.iter().map(|n| (n.id, 0)).collect();
    let mut outgoing: HashMap<_, Vec<_>> = nodes.iter().map(|n| (n.id, vec![])).collect();

    for edge in edges {
        *in_degree.get_mut(&edge.target_node).unwrap() += 1;
        outgoing
            .get_mut(&edge.source_node)
            .unwrap()
            .push(edge.target_node);
    }

    let mut layers: HashMap<Uuid, Layer> = HashMap::new();
    let mut queue: Vec<_> = in_degree
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(id, _)| *id)
        .collect();

    for node_id in &queue {
        layers.insert(*node_id, 0);
    }

    while let Some(node_id) = queue.pop() {
        let current_layer = layers[&node_id];
        for &neighbor in outgoing.get(&node_id).unwrap() {
            let new_layer = current_layer + 1;
            let existing = layers.entry(neighbor).or_insert(new_layer);
            *existing = (*existing).max(new_layer);
            *in_degree.get_mut(&neighbor).unwrap() -= 1;
            if in_degree[&neighbor] == 0 {
                queue.push(neighbor);
            }
        }
    }

    Ok(layers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{Edge, Node};
    use uuid::Uuid;

    #[test]
    fn test_compute_layers() {
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

        let layers = compute_layers(&nodes, &edges).unwrap();

        assert_eq!(layers[&nodes[0].id], 0);
        assert_eq!(layers[&nodes[1].id], 1);
        assert_eq!(layers[&nodes[2].id], 2);
    }
}
