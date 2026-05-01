// Re-export from the main snapshot module for backward compatibility.
//
// The canonical `ExtensionSnapshot` now lives in `crate::snapshot::types`.
// This module provides the original import path (`crate::runtime::snapshot::ExtensionSnapshot`)
// so existing code in `runtime/in_memory.rs` and tests continues to compile without changes.
pub use crate::snapshot::ExtensionSnapshot;
