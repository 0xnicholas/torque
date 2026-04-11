use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    Json,
};
use llm::OpenAiClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::db::Database;
use crate::models::{MemoryCandidate, MemoryEntry, MemoryLayer};

#[derive(Debug, Deserialize)]
pub struct CreateCandidateRequest {
    pub layer: MemoryLayer,
    pub proposed_fact: String,
    pub source_type: Option<String>,
    pub source_ref: Option<String>,
    pub proposer: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct AcceptCandidateResponse {
    pub candidate: MemoryCandidate,
    pub entry: MemoryEntry,
}

#[derive(Debug, Deserialize)]
pub struct ReplaceEntryRequest {
    pub content: String,
    pub layer: Option<MemoryLayer>,
    pub source_type: Option<String>,
    pub source_ref: Option<String>,
    pub proposer: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchMemoryQuery {
    pub q: Option<String>,
    pub limit: Option<i64>,
}

async fn load_session_for_api_key(
    db: &Database,
    session_id: Uuid,
    api_key: &str,
) -> Result<crate::models::Session, StatusCode> {
    let session = crate::db::sessions::get_by_id(db.pool(), session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if !bool::from(session.api_key.as_bytes().ct_eq(api_key.as_bytes())) {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(session)
}

pub async fn create_candidate(
    State((db, _)): State<(Database, Arc<OpenAiClient>)>,
    Path(session_id): Path<Uuid>,
    Extension(api_key): Extension<String>,
    Json(req): Json<CreateCandidateRequest>,
) -> Result<Json<MemoryCandidate>, StatusCode> {
    let session = load_session_for_api_key(&db, session_id, &api_key).await?;

    let mut candidate = MemoryCandidate::new(
        session.project_scope.clone(),
        req.layer,
        req.proposed_fact,
    );
    candidate.source_type = req.source_type;
    candidate.source_ref = req.source_ref;
    candidate.proposer = req.proposer;
    candidate.confidence = req.confidence;

    let saved = crate::db::memory_candidates::create(db.pool(), &candidate)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(saved))
}

pub async fn accept_candidate(
    State((db, _)): State<(Database, Arc<OpenAiClient>)>,
    Path((session_id, candidate_id)): Path<(Uuid, Uuid)>,
    Extension(api_key): Extension<String>,
) -> Result<Json<AcceptCandidateResponse>, StatusCode> {
    let session = load_session_for_api_key(&db, session_id, &api_key).await?;

    let accepted = crate::db::memory_candidates::accept_candidate_to_entry(
        db.pool(),
        &session.project_scope,
        candidate_id,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some((candidate, entry)) = accepted else {
        return Err(StatusCode::NOT_FOUND);
    };

    Ok(Json(AcceptCandidateResponse { candidate, entry }))
}

pub async fn list_entries(
    State((db, _)): State<(Database, Arc<OpenAiClient>)>,
    Path(session_id): Path<Uuid>,
    Extension(api_key): Extension<String>,
) -> Result<Json<Vec<MemoryEntry>>, StatusCode> {
    let session = load_session_for_api_key(&db, session_id, &api_key).await?;

    let entries = crate::db::memory_entries::list_by_project_scope(
        db.pool(),
        &session.project_scope,
        100,
        0,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(entries))
}

pub async fn search_entries(
    State((db, _)): State<(Database, Arc<OpenAiClient>)>,
    Path(session_id): Path<Uuid>,
    Query(query): Query<SearchMemoryQuery>,
    Extension(api_key): Extension<String>,
) -> Result<Json<Vec<MemoryEntry>>, StatusCode> {
    let session = load_session_for_api_key(&db, session_id, &api_key).await?;
    let q = query.q.unwrap_or_default();
    let limit = query.limit.unwrap_or(10).clamp(1, 50);

    let entries =
        crate::db::memory_entries::recall_for_prompt(db.pool(), &session.project_scope, &q, limit)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(entries))
}

pub async fn replace_entry(
    State((db, _)): State<(Database, Arc<OpenAiClient>)>,
    Path((session_id, entry_id)): Path<(Uuid, Uuid)>,
    Extension(api_key): Extension<String>,
    Json(req): Json<ReplaceEntryRequest>,
) -> Result<Json<MemoryEntry>, StatusCode> {
    let session = load_session_for_api_key(&db, session_id, &api_key).await?;

    let old_entry = crate::db::memory_entries::get_by_id(db.pool(), &session.project_scope, entry_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let replaced = crate::db::memory_entries::update_status(
        db.pool(),
        &session.project_scope,
        old_entry.id,
        crate::models::MemoryEntryStatus::Replaced,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if replaced.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let mut new_entry = MemoryEntry::new(
        session.project_scope.clone(),
        req.layer.unwrap_or(old_entry.layer),
        req.content,
    );
    new_entry.source_type = req.source_type.or(Some("memory_replace".to_string()));
    new_entry.source_ref = req
        .source_ref
        .or(Some(format!("memory-entry://{}", old_entry.id)));
    new_entry.proposer = req.proposer;

    let saved = crate::db::memory_entries::create(db.pool(), &new_entry)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(saved))
}

pub async fn invalidate_entry(
    State((db, _)): State<(Database, Arc<OpenAiClient>)>,
    Path((session_id, entry_id)): Path<(Uuid, Uuid)>,
    Extension(api_key): Extension<String>,
) -> Result<Json<MemoryEntry>, StatusCode> {
    let session = load_session_for_api_key(&db, session_id, &api_key).await?;

    let updated = crate::db::memory_entries::update_status(
        db.pool(),
        &session.project_scope,
        entry_id,
        crate::models::MemoryEntryStatus::Invalidated,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some(updated) = updated else {
        return Err(StatusCode::NOT_FOUND);
    };

    Ok(Json(updated))
}
