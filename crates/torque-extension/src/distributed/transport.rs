use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::error::{ExtensionError, Result};
use crate::id::ExtensionId;
use crate::message::ExtensionAction;

// ── Remote Endpoint ──────────────────────────────────────────────────────

/// Describes where a remote Extension is reachable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteEndpoint {
    /// Unique identifier for the node hosting this Extension.  
    /// Used by the registry to distinguish nodes.
    pub node_id: String,
    /// The Extension running on the remote node.
    pub extension_id: ExtensionId,
    /// Hostname or IP address.
    pub address: String,
    /// Port number.
    pub port: u16,
}

// ── Transport Trait ──────────────────────────────────────────────────────

/// Abstract communication layer for cross-process Extension messages.
///
/// Implementations:
/// - [`InProcTransport`] — in-process channels (testing / single-process multi-node)
/// - `RedisTransport` — Redis Streams (requires `redis` crate)
/// - `GrpcTransport` — gRPC (requires `tonic` crate)
#[async_trait]
pub trait Transport: Send + Sync {
    /// Fire-and-forget send to a remote endpoint.
    async fn send(&self, target: &RemoteEndpoint, action: ExtensionAction) -> std::result::Result<(), TransportError>;

    /// Send a request and expect a response (with timeout support via caller).
    async fn call(
        &self,
        target: &RemoteEndpoint,
        action: ExtensionAction,
        timeout: std::time::Duration,
    ) -> std::result::Result<ExtensionAction, TransportError>;

    /// Check whether the transport layer is healthy.
    async fn health_check(&self) -> bool;
}

// ── Transport Error ──────────────────────────────────────────────────────

/// Errors that can occur in the transport layer.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("connection error: {0}")]
    Connection(String),

    #[error("send error: {0}")]
    Send(String),

    #[error("timeout")]
    Timeout,

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("remote error: {0}")]
    Remote(String),
}

impl From<TransportError> for ExtensionError {
    fn from(e: TransportError) -> Self {
        ExtensionError::RuntimeError(e.to_string())
    }
}

// ── In-Process Transport ─────────────────────────────────────────────────

/// An in-memory transport that uses `tokio::sync::mpsc` channels.
///
/// Useful for testing and single-process multi-node simulations.
/// Each "remote" Extension is represented by a channel sender.
pub struct InProcTransport {
    /// node_id → (extension_id → sender)
    channels: tokio::sync::RwLock<
        std::collections::HashMap<String, std::collections::HashMap<ExtensionId, mpsc::UnboundedSender<ExtensionAction>>>,
    >,
    /// node_id → response receiver channels
    response_channels:
        tokio::sync::RwLock<std::collections::HashMap<String, mpsc::UnboundedSender<(ExtensionId, ExtensionAction)>>>,
    node_id: String,
}

impl InProcTransport {
    /// Create a new in-process transport for the given local node.
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            channels: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            response_channels: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            node_id: node_id.into(),
        }
    }

    /// Register a remote node's channel.
    pub async fn register_remote(
        &self,
        node_id: &str,
        extension_id: ExtensionId,
        sender: mpsc::UnboundedSender<ExtensionAction>,
    ) {
        let mut channels = self.channels.write().await;
        channels
            .entry(node_id.to_string())
            .or_default()
            .insert(extension_id, sender);
    }

    /// Register a response channel for a remote node.
    pub async fn register_response_channel(
        &self,
        node_id: &str,
        sender: mpsc::UnboundedSender<(ExtensionId, ExtensionAction)>,
    ) {
        let mut channels = self.response_channels.write().await;
        channels.insert(node_id.to_string(), sender);
    }
}

#[async_trait]
impl Transport for InProcTransport {
    async fn send(&self, target: &RemoteEndpoint, action: ExtensionAction) -> std::result::Result<(), TransportError> {
        let channels = self.channels.read().await;
        let node_channels = channels
            .get(&target.node_id)
            .ok_or_else(|| TransportError::Connection(format!("unknown node: {}", target.node_id)))?;
        let sender = node_channels
            .get(&target.extension_id)
            .ok_or_else(|| TransportError::Connection(format!("unknown extension on node {}", target.node_id)))?;
        sender
            .send(action)
            .map_err(|_| TransportError::Send("channel closed".into()))
    }

    async fn call(
        &self,
        target: &RemoteEndpoint,
        action: ExtensionAction,
        timeout: std::time::Duration,
    ) -> std::result::Result<ExtensionAction, TransportError> {
        // Create a response channel for this call.
        let (tx, mut rx) = mpsc::unbounded_channel();
        self.register_response_channel(&target.node_id, tx).await;

        // Send the request.
        self.send(target, action).await?;

        // Wait for response with timeout.
        tokio::time::timeout(timeout, rx.recv())
            .await
            .map_err(|_| TransportError::Timeout)?
            .ok_or_else(|| TransportError::Send("response channel closed".into()))
            .map(|(_id, action)| action)
    }

    async fn health_check(&self) -> bool {
        // InProcTransport is always healthy as long as it exists.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::ExtensionId;

    #[tokio::test]
    async fn test_inproc_send_and_receive() {
        let transport = InProcTransport::new("node-a");
        let ext_id = ExtensionId::new();

        let (tx, mut rx) = mpsc::unbounded_channel();
        transport
            .register_remote("node-b", ext_id, tx)
            .await;

        let endpoint = RemoteEndpoint {
            node_id: "node-b".into(),
            extension_id: ext_id,
            address: "localhost".into(),
            port: 9090,
        };

        let action = ExtensionAction::Custom { namespace: "torque".into(), name: "ping".into(), payload: serde_json::Value::Null };
        transport.send(&endpoint, action.clone()).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received, action);
    }

    #[tokio::test]
    async fn test_inproc_send_unknown_node() {
        let transport = InProcTransport::new("node-a");
        let ext_id = ExtensionId::new();

        let endpoint = RemoteEndpoint {
            node_id: "unknown".into(),
            extension_id: ext_id,
            address: "localhost".into(),
            port: 0,
        };

        let result = transport
            .send(&endpoint, ExtensionAction::Custom { namespace: "torque".into(), name: "test".into(), payload: serde_json::Value::Null })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_health_check() {
        let transport = InProcTransport::new("node-a");
        assert!(transport.health_check().await);
    }

    #[tokio::test]
    async fn test_remote_endpoint_serde() {
        let ext_id = ExtensionId::new();
        let ep = RemoteEndpoint {
            node_id: "node-x".into(),
            extension_id: ext_id,
            address: "192.168.1.1".into(),
            port: 8080,
        };

        let json = serde_json::to_string(&ep).unwrap();
        let deserialized: RemoteEndpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(ep, deserialized);
    }

    #[test]
    fn test_transport_error_display() {
        let err = TransportError::Timeout;
        assert_eq!(err.to_string(), "timeout");

        let err = TransportError::Connection("refused".into());
        assert_eq!(err.to_string(), "connection error: refused");
    }

    #[test]
    fn test_transport_error_into_extension_error() {
        let err = TransportError::Timeout;
        let ext_err: ExtensionError = err.into();
        assert!(matches!(ext_err, ExtensionError::RuntimeError(_)));
    }
}
