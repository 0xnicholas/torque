pub mod scheduler;
pub mod worker;
pub mod crash_recovery;
pub mod error;

pub use error::ExecutorError;
pub use scheduler::Scheduler;
pub use worker::Worker;

use std::sync::Arc;
use tokio::sync::Semaphore;
use db::PgPool;
use redis::aio::ConnectionManager;
use checkpointer::Checkpointer;

pub struct ExecutorConfig {
    pub executor_id: String,
    pub worker_pool_size: usize,
    pub lock_timeout_secs: u64,
    pub crash_recovery_interval_secs: u64,
    pub tenant_id: uuid::Uuid,
}

pub struct Executor<C: Checkpointer> {
    config: ExecutorConfig,
    pool: PgPool,
    redis: Option<ConnectionManager>,
    scheduler: Scheduler,
    worker_semaphore: Arc<Semaphore>,
    checkpointer: Option<Arc<C>>,
}

impl<C: Checkpointer + 'static> Executor<C> {
    pub async fn new(config: ExecutorConfig, checkpointer: Option<Arc<C>>) -> Result<Self, ExecutorError> {
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| ExecutorError::Config("DATABASE_URL not set".to_string()))?;
        
        let pool = PgPool::connect(&database_url)
            .await
            .map_err(|e| ExecutorError::Database(e.to_string()))?;
        
        let redis = if let Ok(redis_url) = std::env::var("REDIS_URL") {
            let client = redis::Client::open(redis_url)
                .map_err(|e| ExecutorError::Config(format!("Redis error: {}", e)))?;
            Some(
                ConnectionManager::new(client)
                    .await
                    .map_err(|e| ExecutorError::Config(format!("Redis connection error: {}", e)))?,
            )
        } else {
            None
        };
        
        let tenants = db::tenants::list(&pool)
            .await
            .map_err(|e| ExecutorError::Database(e.to_string()))?;
        let scheduler = Scheduler::new(tenants);
        
        let worker_pool_size = config.worker_pool_size;
        
        Ok(Self {
            config,
            pool,
            redis,
            scheduler,
            worker_semaphore: Arc::new(Semaphore::new(worker_pool_size)),
            checkpointer,
        })
    }
    
    pub async fn run(&mut self) -> Result<(), ExecutorError> {
        tracing::info!(executor_id = %self.config.executor_id, "Starting executor");
        
        let crash_recovery_interval = tokio::time::Duration::from_secs(self.config.crash_recovery_interval_secs);
        let mut crash_recovery_timer = tokio::time::interval(crash_recovery_interval);
        
        loop {
            tokio::select! {
                _ = crash_recovery_timer.tick() => {
                    self.run_crash_recovery().await;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    if let Err(e) = self.try_dispatch_node().await {
                        tracing::error!("Error dispatching node: {}", e);
                    }
                }
            }
        }
    }
    
    async fn run_crash_recovery(&self) {
        tracing::debug!("Running crash recovery scan");
        
        let stale_entries = match crash_recovery::find_stale_locked_entries(&self.pool, 600).await {
            Ok(entries) => entries,
            Err(e) => {
                tracing::error!("Failed to find stale entries: {}", e);
                return;
            }
        };
        
        let checkpointer_ref = self.checkpointer.as_ref().map(|c| c.as_ref() as &dyn Checkpointer);
        
        for entry in stale_entries {
            tracing::info!(entry_id = %entry.id, "Found stale locked entry");
            
            let recovered = match crash_recovery::recover_node_if_needed(&self.pool, entry.node_id, entry.id, checkpointer_ref).await {
                Ok(recovered) => recovered,
                Err(e) => {
                    tracing::error!("Failed to recover node {}: {}", entry.node_id, e);
                    continue;
                }
            };
            
            if recovered {
                tracing::info!(node_id = %entry.node_id, "Node recovered, marked as done");
            } else {
                if let Err(e) = crash_recovery::reset_stale_entry(&self.pool, &entry).await {
                    tracing::error!("Failed to reset stale entry {}: {}", entry.id, e);
                }
            }
        }
    }
    
    async fn try_dispatch_node(&mut self) -> Result<(), ExecutorError> {
        let tenant_id = match self.scheduler.next() {
            Some(id) => id,
            None => return Ok(()),
        };
        
        if let Some(ref redis) = self.redis {
            let tenant = db::tenants::get(&self.pool, tenant_id)
                .await
                .map_err(|e| ExecutorError::Database(e.to_string()))?;
            
            if let Some(tenant) = tenant {
                let concurrency_key = format!("{}:run:current:concurrency", tenant_id);
                let current_concurrency: i64 = redis::cmd("GET")
                    .arg(&concurrency_key)
                    .query_async(&mut redis.clone())
                    .await
                    .unwrap_or(0);
                
                if current_concurrency >= tenant.max_concurrency as i64 {
                    tracing::debug!(tenant_id = %tenant_id, "Tenant at max concurrency");
                    return Ok(());
                }
                
                if let Some(quota) = tenant.monthly_token_quota {
                    let token_key = format!("{}:token_usage:monthly", tenant_id);
                    let current_usage: i64 = redis::cmd("GET")
                        .arg(&token_key)
                        .query_async(&mut redis.clone())
                        .await
                        .unwrap_or(0);
                    
                    if current_usage >= quota {
                        tracing::debug!(tenant_id = %tenant_id, "Tenant at token quota limit");
                        return Ok(());
                    }
                }
            }
        }
        
        let permit = self.worker_semaphore.clone().acquire_owned().await
            .map_err(|e| ExecutorError::Other(e.to_string()))?;
        
        let entry = queue::dequeue(&self.pool, tenant_id, &self.config.executor_id)
            .await
            .map_err(|e| ExecutorError::Queue(e.to_string()))?;
        
        if let Some(e) = entry {
            let node = db::nodes::get(&self.pool, e.node_id)
                .await
                .map_err(|e| ExecutorError::Database(e.to_string()))?;
            
            if let Some(node) = node {
                if !Worker::check_upstream_deps_done(&self.pool, node.id).await
                    .map_err(|e| ExecutorError::Database(e.to_string()))?
                {
                    tracing::debug!(node_id = %node.id, "Upstream dependencies not satisfied");
                    drop(permit);
                    return Ok(());
                }
                
                if let Some(ref redis) = self.redis {
                    let concurrency_key = format!("{}:run:current:concurrency", tenant_id);
                    let _: Result<i64, _> = redis::cmd("INCR")
                        .arg(&concurrency_key)
                        .query_async(&mut redis.clone())
                        .await;
                }
                
                let pool = self.pool.clone();
                let node_id = node.id;
                
                let queue_id = e.id;
                let tenant_id_for_decr = tenant_id;
                let redis_for_decr = self.redis.clone();
                
                tokio::spawn(async move {
                    let result = Worker::new(0, pool.clone()).run_node(node).await;
                    
                    if let Err(e) = result {
                        tracing::error!(node_id = %node_id, "Node execution failed: {}", e);
                    }
                    
                    let _ = queue::complete(&pool, queue_id).await;
                    
                    if let Some(redis) = redis_for_decr {
                        let concurrency_key = format!("{}:run:current:concurrency", tenant_id_for_decr);
                        let _: Result<i64, _> = redis::cmd("DECR")
                            .arg(&concurrency_key)
                            .query_async(&mut redis.clone())
                            .await;
                    }
                    
                    drop(permit);
                });
            }
        }
        
        Ok(())
    }
}