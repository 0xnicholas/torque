use axum::Json;

pub async fn get() -> Json<crate::metrics::RuntimeMetricsSnapshot> {
    Json(crate::metrics::snapshot())
}
