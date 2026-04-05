use std::collections::VecDeque;
use types::Tenant;

#[derive(Clone)]
pub struct Scheduler {
    tenant_ids: VecDeque<uuid::Uuid>,
    current_index: usize,
}

impl Scheduler {
    pub fn new(tenants: Vec<Tenant>) -> Self {
        let mut expanded_ids: Vec<_> = tenants
            .iter()
            .flat_map(|t| std::iter::repeat(t.id).take(t.weight.max(1) as usize))
            .collect();
        expanded_ids.sort();

        Self {
            tenant_ids: VecDeque::from(expanded_ids),
            current_index: 0,
        }
    }

    pub fn next(&mut self) -> Option<uuid::Uuid> {
        if self.tenant_ids.is_empty() {
            return None;
        }

        let len = self.tenant_ids.len();
        let id = self.tenant_ids[self.current_index];
        self.current_index = (self.current_index + 1) % len;
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::Tenant;
    use uuid::Uuid;

    #[test]
    fn test_scheduler_round_robin() {
        let tenant1 = Tenant {
            id: Uuid::new_v4(),
            name: "tenant1".to_string(),
            weight: 1,
            max_concurrency: 10,
            monthly_token_quota: None,
            created_at: chrono::Utc::now(),
        };

        let tenant2 = Tenant {
            id: Uuid::new_v4(),
            name: "tenant2".to_string(),
            weight: 1,
            max_concurrency: 10,
            monthly_token_quota: None,
            created_at: chrono::Utc::now(),
        };

        let mut scheduler = Scheduler::new(vec![tenant1.clone(), tenant2.clone()]);

        let first = scheduler.next();
        let second = scheduler.next();

        assert_ne!(first, second);
    }

    #[test]
    fn test_scheduler_weighted_round_robin() {
        let tenant1 = Tenant {
            id: Uuid::new_v4(),
            name: "tenant1".to_string(),
            weight: 2,
            max_concurrency: 10,
            monthly_token_quota: None,
            created_at: chrono::Utc::now(),
        };

        let tenant2 = Tenant {
            id: Uuid::new_v4(),
            name: "tenant2".to_string(),
            weight: 1,
            max_concurrency: 10,
            monthly_token_quota: None,
            created_at: chrono::Utc::now(),
        };

        let mut scheduler = Scheduler::new(vec![tenant1.clone(), tenant2.clone()]);

        let t1_id = tenant1.id;
        let t2_id = tenant2.id;

        let results: Vec<_> = (0..6).map(|_| scheduler.next()).collect();

        let t1_count = results.iter().filter(|&&r| r == Some(t1_id)).count();
        let t2_count = results.iter().filter(|&&r| r == Some(t2_id)).count();

        assert_eq!(t1_count, 4);
        assert_eq!(t2_count, 2);
    }

    #[test]
    fn test_scheduler_empty() {
        let mut scheduler = Scheduler::new(vec![]);
        assert_eq!(scheduler.next(), None);
    }
}
