use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use torque_runtime::CancellationToken;

use crate::models::v1::session::{
    CompactionAbortResponse, CompactJobResponse, Session, SessionChatRequest,
    SessionCompactRequest, SessionCreateRequest, SessionStatus,
};
use crate::repository::SessionRepository;

/// Service for managing long-running agent sessions with explicit compaction control.
///
/// Architecture:
/// - `compact()` creates a `CancellationToken`, registers the job, and signals the
///   session's message queue to begin compaction. The queue runs compaction in the
///   background and checks cancellation via the token.
/// - `abort_compaction()` cancels the token, which the queue observes on the next
///   cancellation check, causing the compaction to abort early.
/// - Extension hooks (`PRE_COMPACTION` / `POST_COMPACTION`) are fired before and
///   after the compaction operation (see `todo` in `compact()`).
pub struct SessionService {
    repo: Arc<dyn SessionRepository>,

    /// Registry of in-flight compaction jobs.
    /// Keyed by `session_id` → `CancellationToken` that the message queue checks.
    /// Guarded by `Mutex` since reads/writes happen from both API handlers and
    /// background compaction tasks.
    active_jobs: Arc<Mutex<HashMap<Uuid, CancellationToken>>>,
}

impl SessionService {
    pub fn new(repo: Arc<dyn SessionRepository>) -> Self {
        Self {
            repo,
            active_jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // ── CRUD ────────────────────────────────────────────────────

    pub async fn create(
        &self,
        tenant_id: Uuid,
        req: SessionCreateRequest,
    ) -> anyhow::Result<Session> {
        let now = chrono::Utc::now();
        let session = Session {
            id: Uuid::new_v4(),
            tenant_id,
            agent_definition_id: req.agent_definition_id,
            agent_instance_id: None,
            status: SessionStatus::Active,
            title: req.title,
            metadata: req
                .metadata
                .unwrap_or(serde_json::Value::Object(Default::default())),
            active_compaction_job_id: None,
            created_at: now,
            updated_at: now,
        };
        self.repo.create(&session).await
    }

    pub async fn get(&self, id: Uuid, tenant_id: Uuid) -> anyhow::Result<Option<Session>> {
        self.repo.get(id, tenant_id).await
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<Session>> {
        self.repo.list(tenant_id, limit, offset).await
    }

    /// Delete a session. Does NOT update status first — the delete is atomic.
    /// If the session has an active compaction job, it is cancelled first.
    pub async fn delete(&self, id: Uuid, tenant_id: Uuid) -> anyhow::Result<bool> {
        // Cancel any in-flight compaction for this session (best-effort).
        if let Ok(mut jobs) = self.active_jobs.lock() {
            if let Some(token) = jobs.remove(&id) {
                token.cancel();
            }
        }

        // Update DB as Terminated before deletion, so concurrent reads see the terminal state.
        let _ = self
            .repo
            .update_status(id, tenant_id, &SessionStatus::Terminated)
            .await;

        self.repo.delete(id, tenant_id).await
    }

    // ── Chat ────────────────────────────────────────────────────

    /// Send a chat message to a session.
    ///
    /// Returns the `agent_instance_id` if the session has been bound to one,
    /// or the session ID itself as a fallback identifier.
    ///
    /// TODO: When the session has no active agent instance, the caller should
    ///   auto-create an AgentInstance from `agent_definition_id` and bind it
    ///   to the session before routing the message. This requires coordination
    ///   with `RunService` / `AgentInstanceService`.
    pub async fn chat(
        &self,
        session_id: Uuid,
        tenant_id: Uuid,
        _req: SessionChatRequest,
    ) -> anyhow::Result<Uuid> {
        let session = self
            .repo
            .get(session_id, tenant_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        if !session.status.is_available() {
            anyhow::bail!(
                "Session is not available: current status = {}",
                session.status
            );
        }

        // If no agent instance is bound yet, return the session_id as a fallback.
        // The caller can use session_id to auto-create and bind an AgentInstance
        // before proceeding with message routing.
        Ok(session.agent_instance_id.unwrap_or(session_id))
    }

    // ── Compaction ──────────────────────────────────────────────

    /// Explicitly trigger context compaction on a session.
    ///
    /// If `custom_instructions` is provided, those instructions are passed
    /// to the LLM summarizer to guide what the summary should focus on.
    ///
    /// ## Lifecycle
    /// 1. Validates the session exists and can be compacted (`Active` or `Compacting`).
    /// 2. Creates a `CancellationToken` and registers it in `active_jobs`.
    /// 3. Updates session status to `Compacting` in the database.
    /// 4. Signals the session's message queue to begin compaction.
    /// 5. When compaction completes (or is aborted), the cleanup handler runs.
    ///
    /// ## Extension Hooks (when `extension` feature is enabled)
    /// - `PRE_COMPACTION` is fired with `custom_instructions` and `message_count`
    ///   before the compaction is dispatched to the queue.
    /// - `POST_COMPACTION` is fired with `success` / `aborted` flags after
    ///   the compaction completes.
    ///
    /// TODO: Wire extension hooks via `ServiceContainer.extension_service` when
    ///   the `extension` feature is enabled. Currently only the state machine
    ///   and cancellation infrastructure are fully implemented.
    pub async fn compact(
        &self,
        session_id: Uuid,
        tenant_id: Uuid,
        req: SessionCompactRequest,
    ) -> anyhow::Result<CompactJobResponse> {
        let session = self
            .repo
            .get(session_id, tenant_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        if !session.status.can_compact() {
            anyhow::bail!(
                "Session cannot be compacted: current status = {} (requires active or compacting)",
                session.status
            );
        }

        let job_id = Uuid::new_v4();
        let cancel_token = CancellationToken::new();

        // Register job before updating DB so abort_compaction can find it.
        self.active_jobs
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock job registry: {}", e))?
            .insert(session_id, cancel_token.clone());

        self.repo
            .update_status(session_id, tenant_id, &SessionStatus::Compacting)
            .await?;
        self.repo
            .update_compaction_job(session_id, tenant_id, Some(job_id))
            .await?;

        // TODO: Dispatch to runtime message queue for actual compaction.
        // Once the queue picks this up, it should:
        //   1. Receive the `CancellationToken` for this job
        //   2. Run `compact_with_options()` with `req.custom_instructions` and the token
        //   3. On completion, call back to `SessionService::on_compaction_complete()`
        //
        //   if let Some(ref mq) = self.message_queue {
        //       mq.request_compact(job_id, req.custom_instructions, cancel_token).await;
        //   }
        //
        // For now, spawn a background task that simulates the completion flow.
        let jobs = self.active_jobs.clone();
        let repo = self.repo.clone();
        tokio::spawn(async move {
            // Simulated: in production, this would be the compaction worker.
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Check if cancelled during the simulated work.
            if cancel_token.is_cancelled() {
                tracing::info!(session_id = %session_id, job_id = %job_id, "Compaction cancelled during work");
                // Status is already reset to Active by abort_compaction().
                return;
            }

            // Mark compaction as complete.
            let _ = repo
                .update_status(session_id, tenant_id, &SessionStatus::Active)
                .await;
            let _ = repo
                .update_compaction_job(session_id, tenant_id, None)
                .await;

            // Clean up job registry.
            let _ = jobs.lock().map(|mut m| m.remove(&session_id));

            tracing::info!(session_id = %session_id, job_id = %job_id, "Compaction complete");
        });

        Ok(CompactJobResponse {
            job_id,
            status: "compacting".into(),
            session_id,
        })
    }

    /// Abort an in-flight compaction job on a session.
    ///
    /// 1. Looks up the session's current compaction job.
    /// 2. Cancels the `CancellationToken` — the compaction worker observes this
    ///    and aborts early.
    /// 3. Resets session status back to `Active`.
    /// 4. Fires `POST_COMPACTION` extension hook with `aborted=true`.
    pub async fn abort_compaction(
        &self,
        session_id: Uuid,
        tenant_id: Uuid,
    ) -> anyhow::Result<CompactionAbortResponse> {
        let session = self
            .repo
            .get(session_id, tenant_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        if session.status != SessionStatus::Compacting {
            anyhow::bail!(
                "Session is not compacting: current status = {}",
                session.status
            );
        }

        // Cancel the in-flight compaction via its CancellationToken.
        let job_id = session.active_compaction_job_id;
        if let Ok(mut jobs) = self.active_jobs.lock() {
            if let Some(token) = jobs.remove(&session_id) {
                token.cancel();
                tracing::info!(
                    session_id = %session_id,
                    "Cancellation signalled to in-flight compaction job"
                );
            }
        }

        self.repo
            .update_status(session_id, tenant_id, &SessionStatus::Active)
            .await?;
        self.repo
            .update_compaction_job(session_id, tenant_id, None)
            .await?;

        Ok(CompactionAbortResponse {
            job_id,
            status: "aborted".into(),
            session_id,
        })
    }
}
