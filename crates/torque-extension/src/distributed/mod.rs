//! # Distributed Extension Support
//!
//! Provides abstractions for running Extensions across multiple nodes:
//!
//! - [`Transport`] — Trait for cross-process communication (in-process, Redis, gRPC)
//! - [`ServiceRegistry`] — Trait for discovering where Extensions are located
//! - [`RemoteExtensionRuntime`] — Proxy runtime that routes to local or remote
//! - [`MessageRouter`] — Routes messages between local and remote Extensions
//! - [`LoadBalancer`] — Strategies for selecting target Extensions
//!
//! ## Architecture
//!
//! ```text
//!  RemoteExtensionRuntime
//!     ├── local: InMemoryExtensionRuntime  (local Extensions)
//!     ├── transport: Arc<dyn Transport>     (cross-process I/O)
//!     └── registry: Arc<dyn ServiceRegistry> (location discovery)
//! ```
//!
//! All in-memory implementations are provided so the distributed layer can
//! be tested without external dependencies (Redis, gRPC, etc.).

pub mod load_balancer;
pub mod registry;
pub mod remote;
pub mod router;
pub mod transport;

pub use load_balancer::{LoadBalancer, LoadBalancingStrategy};
pub use registry::{InMemoryServiceRegistry, ServiceRegistry};
pub use remote::RemoteExtensionRuntime;
pub use router::MessageRouter;
pub use transport::{InProcTransport, RemoteEndpoint, Transport};
