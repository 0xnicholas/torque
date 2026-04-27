use async_trait::async_trait;
use chrono::{DateTime, Utc};
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    pub id: Option<String>,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

impl StreamMessage {
    pub fn new(key: &str, data: serde_json::Value) -> Self {
        Self {
            id: None,
            data: serde_json::json!({
                "key": key,
                "data": data,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
            timestamp: chrono::Utc::now(),
        }
    }
}

#[async_trait]
pub trait StreamBus: Send + Sync {
    async fn xadd(&self, stream: &str, message: &StreamMessage) -> anyhow::Result<String>;
    async fn xread(
        &self,
        streams: &[(&str, &str)],
        count: usize,
    ) -> anyhow::Result<Vec<StreamReadResult>>;
    async fn xreadgroup(
        &self,
        group: &str,
        consumer: &str,
        streams: &[(&str, &str)],
        count: usize,
    ) -> anyhow::Result<Vec<StreamReadResult>>;
    async fn xack(&self, stream: &str, group: &str, ids: &[&str]) -> anyhow::Result<()>;
    async fn create_consumer_group(
        &self,
        stream: &str,
        group: &str,
        start_id: &str,
    ) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct StreamReadResult {
    pub stream: String,
    pub id: String,
    pub data: serde_json::Value,
}

pub struct RedisStreamBus {
    conn: ConnectionManager,
}

impl RedisStreamBus {
    pub fn new(conn: ConnectionManager) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl StreamBus for RedisStreamBus {
    async fn xadd(&self, stream: &str, message: &StreamMessage) -> anyhow::Result<String> {
        let mut conn = self.conn.clone();

        let mut args = vec![stream.to_string(), "*".to_string()];

        if let serde_json::Value::Object(obj) = &message.data {
            for (key, value) in obj {
                args.push(key.clone());
                args.push(serde_json::to_string(&value)?);
            }
        } else {
            args.push("data".to_string());
            args.push(serde_json::to_string(&message.data)?);
        }

        let id: String = redis::cmd("XADD").arg(&args).query_async(&mut conn).await?;
        Ok(id)
    }

    async fn xread(
        &self,
        streams: &[(&str, &str)],
        count: usize,
    ) -> anyhow::Result<Vec<StreamReadResult>> {
        let mut conn = self.conn.clone();
        let mut args: Vec<String> = vec!["COUNT".to_string(), count.to_string()];
        for (s, id) in streams {
            args.push(s.to_string());
            args.push(id.to_string());
        }
        let result: Vec<(String, Vec<(String, Vec<(String, String)>)>)> = redis::cmd("XREAD")
            .arg(&args)
            .query_async(&mut conn)
            .await?;

        let mut results = Vec::new();
        for (stream_name, entries) in result {
            for (entry_id, fields) in entries {
                let mut obj = serde_json::Map::new();
                for (key, value) in fields {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&value) {
                        obj.insert(key, parsed);
                    } else {
                        obj.insert(key, serde_json::Value::String(value));
                    }
                }
                results.push(StreamReadResult {
                    stream: stream_name.clone(),
                    id: entry_id,
                    data: serde_json::Value::Object(obj),
                });
            }
        }
        Ok(results)
    }

    async fn xreadgroup(
        &self,
        group: &str,
        consumer: &str,
        streams: &[(&str, &str)],
        count: usize,
    ) -> anyhow::Result<Vec<StreamReadResult>> {
        let mut conn = self.conn.clone();
        let mut args: Vec<String> = vec![
            "GROUP".to_string(),
            group.to_string(),
            consumer.to_string(),
            "COUNT".to_string(),
            count.to_string(),
        ];
        for (s, id) in streams {
            args.push(s.to_string());
            args.push(id.to_string());
        }
        let result: Vec<(String, Vec<(String, Vec<(String, String)>)>)> = redis::cmd("XREADGROUP")
            .arg(&args)
            .query_async(&mut conn)
            .await?;

        let mut results = Vec::new();
        for (stream_name, entries) in result {
            for (entry_id, fields) in entries {
                let mut obj = serde_json::Map::new();
                for (key, value) in fields {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&value) {
                        obj.insert(key, parsed);
                    } else {
                        obj.insert(key, serde_json::Value::String(value));
                    }
                }
                results.push(StreamReadResult {
                    stream: stream_name.clone(),
                    id: entry_id,
                    data: serde_json::Value::Object(obj),
                });
            }
        }
        Ok(results)
    }

    async fn xack(&self, stream: &str, group: &str, ids: &[&str]) -> anyhow::Result<()> {
        let mut conn = self.conn.clone();
        let mut args = vec![stream.to_string(), group.to_string()];
        args.extend(ids.iter().map(|s| s.to_string()));
        let _: () = redis::cmd("XACK").arg(&args).query_async(&mut conn).await?;
        Ok(())
    }

    async fn create_consumer_group(
        &self,
        stream: &str,
        group: &str,
        start_id: &str,
    ) -> anyhow::Result<()> {
        let mut conn = self.conn.clone();
        let _: () = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(stream)
            .arg(group)
            .arg(start_id)
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create consumer group: {}", e))?;
        Ok(())
    }
}
