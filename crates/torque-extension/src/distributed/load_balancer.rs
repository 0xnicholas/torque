use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{atomic::{AtomicUsize, Ordering}, Mutex};

use crate::id::ExtensionId;

/// Load-balancing strategies for distributing messages across multiple
/// Extension instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalancingStrategy {
    /// Distribute in round-robin order.
    RoundRobin,
    /// Select the Extension with the fewest active connections.
    LeastConnections,
    /// Select a random Extension.
    Random,
    /// Weighted random selection.
    WeightedRandom,
    /// Consistent hashing (key-based routing).
    ConsistentHash,
}

/// A load balancer that selects target Extensions based on the configured strategy.
///
/// # Example
///
/// ```rust,no_run
/// use torque_extension::distributed::{LoadBalancer, LoadBalancingStrategy};
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() {
/// let lb = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
/// let ids = vec![
///     torque_extension::ExtensionId::new(),
///     torque_extension::ExtensionId::new(),
/// ];
/// let selected = lb.select(&ids).await;
/// assert!(selected.is_some());
/// # }
/// ```
pub struct LoadBalancer {
    strategy: LoadBalancingStrategy,
    /// Per-Extension active connection counts.
    connection_counts: Mutex<HashMap<ExtensionId, usize>>,
    /// Per-Extension weights (used by WeightedRandom).
    weights: Mutex<HashMap<ExtensionId, u32>>,
    /// Round-robin counter.
    round_robin_counter: AtomicUsize,
}

impl LoadBalancer {
    /// Create a new load balancer with the given strategy.
    pub fn new(strategy: LoadBalancingStrategy) -> Self {
        Self {
            strategy,
            connection_counts: Mutex::new(HashMap::new()),
            weights: Mutex::new(HashMap::new()),
            round_robin_counter: AtomicUsize::new(0),
        }
    }

    /// Select a target Extension from the provided list.
    pub async fn select(&self, targets: &[ExtensionId]) -> Option<ExtensionId> {
        if targets.is_empty() {
            return None;
        }

        match self.strategy {
            LoadBalancingStrategy::RoundRobin => {
                let counter = self.round_robin_counter.fetch_add(1, Ordering::Relaxed);
                Some(targets[counter % targets.len()])
            }

            LoadBalancingStrategy::LeastConnections => {
                let counts = self.connection_counts.lock().unwrap();
                targets
                    .iter()
                    .min_by_key(|id| counts.get(*id).copied().unwrap_or(0))
                    .copied()
            }

            LoadBalancingStrategy::Random => {
                let hash = {
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .hash(&mut hasher);
                    hasher.finish()
                };
                let index = (hash as usize) % targets.len();
                Some(targets[index])
            }

            LoadBalancingStrategy::WeightedRandom => {
                let weights = self.weights.lock().unwrap();
                let total_weight: u32 = targets
                    .iter()
                    .map(|id| weights.get(id).copied().unwrap_or(1))
                    .sum();

                let hash = {
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .hash(&mut hasher);
                    hasher.finish()
                };
                let mut rand = (hash % total_weight as u64) as i64;

                for id in targets {
                    let weight = weights.get(id).copied().unwrap_or(1) as i64;
                    rand -= weight;
                    if rand < 0 {
                        return Some(*id);
                    }
                }
                Some(targets[0])
            }

            LoadBalancingStrategy::ConsistentHash => {
                // Simplified: return first target.
                // A real implementation would hash the key and map to a ring.
                Some(targets[0])
            }
        }
    }

    /// Record that a connection was dispatched to the given Extension.
    pub async fn record_connection(&self, id: ExtensionId) {
        let mut counts = self.connection_counts.lock().unwrap();
        *counts.entry(id).or_insert(0) += 1;
    }

    /// Release a connection from the given Extension.
    pub async fn release_connection(&self, id: ExtensionId) {
        let mut counts = self.connection_counts.lock().unwrap();
        if let Some(count) = counts.get_mut(&id) {
            *count = count.saturating_sub(1);
        }
    }

    /// Set a weight for the given Extension (used by WeightedRandom).
    pub async fn set_weight(&self, id: ExtensionId, weight: u32) {
        let mut weights = self.weights.lock().unwrap();
        weights.insert(id, weight);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ids(n: usize) -> Vec<ExtensionId> {
        (0..n).map(|_| ExtensionId::new()).collect()
    }

    #[tokio::test]
    async fn test_round_robin_cycles() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
        let ids = make_ids(3);

        let selected = lb.select(&ids).await.unwrap();
        assert!(ids.contains(&selected));

        // Repeated calls should cycle through targets.
        let mut results = Vec::new();
        for _ in 0..6 {
            results.push(lb.select(&ids).await.unwrap());
        }
        // With 3 targets and 6 selections, each should appear exactly twice.
        for id in &ids {
            assert_eq!(results.iter().filter(|&&r| r == *id).count(), 2);
        }
    }

    #[tokio::test]
    async fn test_least_connections_prefers_less_loaded() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::LeastConnections);
        let ids = make_ids(2);

        // Record more connections for the first extension.
        lb.record_connection(ids[0]).await;
        lb.record_connection(ids[0]).await;

        // The second extension should be preferred.
        let selected = lb.select(&ids).await.unwrap();
        assert_eq!(selected, ids[1]);
    }

    #[tokio::test]
    async fn test_random_selects_valid_target() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::Random);
        let ids = make_ids(5);

        for _ in 0..20 {
            let selected = lb.select(&ids).await.unwrap();
            assert!(ids.contains(&selected));
        }
    }

    #[tokio::test]
    async fn test_weighted_random_respects_weights() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::WeightedRandom);
        let ids = make_ids(2);

        lb.set_weight(ids[0], 1).await;
        lb.set_weight(ids[1], 10).await;

        // The second extension should be selected more often (10x weight).
        let mut count_second = 0;
        for _ in 0..100 {
            if lb.select(&ids).await.unwrap() == ids[1] {
                count_second += 1;
            }
        }
        // With 10:1 weight ratio, second should be chosen at least 80% of the time.
        assert!(count_second > 80);
    }

    #[tokio::test]
    async fn test_consistent_hash_returns_first() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::ConsistentHash);
        let ids = make_ids(3);

        let selected = lb.select(&ids).await.unwrap();
        assert!(ids.contains(&selected));
    }

    #[tokio::test]
    async fn test_select_empty_returns_none() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
        let selected = lb.select(&[]).await;
        assert!(selected.is_none());
    }

    #[tokio::test]
    async fn test_connection_tracking() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::LeastConnections);
        let id = ExtensionId::new();

        lb.record_connection(id).await;
        lb.record_connection(id).await;

        {
            let counts = lb.connection_counts.lock().unwrap();
            assert_eq!(counts.get(&id), Some(&2));
        }

        lb.release_connection(id).await;

        {
            let counts = lb.connection_counts.lock().unwrap();
            assert_eq!(counts.get(&id), Some(&1));
        }
    }

    #[tokio::test]
    async fn test_set_weight() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::WeightedRandom);
        let id = ExtensionId::new();

        lb.set_weight(id, 5).await;
        let weights = lb.weights.lock().unwrap();
        assert_eq!(weights.get(&id), Some(&5));
    }
}
