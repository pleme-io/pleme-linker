//! Unit tests for the web bundler module
//!
//! Tests are organized into submodules by category:
//! - `imports` - Import transformation tests
//! - `exports` - Export transformation tests
//! - `typescript` - TypeScript-specific tests (type stripping, interfaces)
//! - `path_resolution` - Path resolution for local modules
//! - `import_meta` - Vite import.meta transformations
//! - `externals` - Configurable externals and CDN loading
//! - `edge_cases` - Real-world patterns and edge cases
//! - `tree_shaking` - Tree shaking and used exports tracking
//! - `interop` - CommonJS/ESM interoperability

mod edge_cases;
mod exports;
mod externals;
mod import_meta;
mod imports;
mod interop;
mod path_resolution;
mod tree_shaking;
mod typescript;

use super::*;
use std::collections::HashSet;
use std::path::PathBuf;

/// Helper function for tests - calls transform_module_code with no path context
/// This is used for unit tests where we're testing the transformation logic
/// without needing real module resolution
pub fn transform_module_code(code: &str) -> String {
    super::transform_module_code(code, None, &HashSet::new())
}

#[test]
fn test_generate_hash() {
    let hash1 = generate_hash("hello world");
    let hash2 = generate_hash("hello world");
    let hash3 = generate_hash("different content");

    assert_eq!(hash1, hash2);
    assert_ne!(hash1, hash3);
}
