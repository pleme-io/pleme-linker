//! Tree shaking tests
//!
//! Tests for tracking used exports across the module graph
//! and verifying that the tree shaking infrastructure works correctly.

use super::*;
use crate::web_bundler::UsedExportsTracker;

// =====================================================
// Used Exports Tracker Tests
// =====================================================

#[test]
fn test_used_exports_tracker_mark_used() {
    let mut tracker = UsedExportsTracker::new();
    let module_path = PathBuf::from("/project/src/utils.ts");

    tracker.mark_used(&module_path, "formatDate");
    tracker.mark_used(&module_path, "parseDate");

    assert!(
        tracker.is_used(&module_path, "formatDate"),
        "formatDate should be marked as used"
    );
    assert!(
        tracker.is_used(&module_path, "parseDate"),
        "parseDate should be marked as used"
    );
    assert!(
        !tracker.is_used(&module_path, "unusedFunction"),
        "unusedFunction should not be marked as used"
    );
}

#[test]
fn test_used_exports_tracker_mark_all() {
    let mut tracker = UsedExportsTracker::new();
    let module_path = PathBuf::from("/project/src/utils.ts");

    tracker.mark_all_used(&module_path);

    // When all are marked, any export should be considered used
    assert!(
        tracker.is_used(&module_path, "anything"),
        "Any export should be used when * is marked"
    );
    assert!(
        tracker.is_used(&module_path, "default"),
        "default should be used when * is marked"
    );
}

#[test]
fn test_used_exports_tracker_has_any() {
    let mut tracker = UsedExportsTracker::new();
    let used_module = PathBuf::from("/project/src/used.ts");
    let unused_module = PathBuf::from("/project/src/unused.ts");

    tracker.mark_used(&used_module, "something");

    assert!(
        tracker.has_any_used(&used_module),
        "used_module should have used exports"
    );
    assert!(
        !tracker.has_any_used(&unused_module),
        "unused_module should not have used exports"
    );
}

#[test]
fn test_used_exports_tracker_multiple_modules() {
    let mut tracker = UsedExportsTracker::new();
    let module_a = PathBuf::from("/project/src/a.ts");
    let module_b = PathBuf::from("/project/src/b.ts");

    tracker.mark_used(&module_a, "funcA");
    tracker.mark_used(&module_b, "funcB");
    tracker.mark_all_used(&module_b);

    assert!(tracker.is_used(&module_a, "funcA"), "funcA should be used");
    assert!(
        !tracker.is_used(&module_a, "funcB"),
        "funcB not in module_a"
    );
    assert!(
        tracker.is_used(&module_b, "anything"),
        "anything in module_b"
    );
}

#[test]
fn test_used_exports_tracker_default_always_used() {
    let mut tracker = UsedExportsTracker::new();
    let module_path = PathBuf::from("/project/src/component.tsx");

    // When default is marked, everything should be considered used
    // because we can't know what the default might reference
    tracker.mark_used(&module_path, "default");

    assert!(
        tracker.is_used(&module_path, "default"),
        "default should be used"
    );
    assert!(
        tracker.is_used(&module_path, "helper"),
        "helper should be used when default is used"
    );
}

// =====================================================
// Module Analysis Tests
// =====================================================

#[test]
fn test_module_exports_detection() {
    // Test that we can detect exports from ESM syntax
    let code = r#"
export const foo = 1;
export function bar() {}
export class Baz {}
export default App;
export { x, y as z };
"#;

    // We test via transform since exports are detected during module analysis
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("module.exports.foo"),
        "Should export foo"
    );
    assert!(
        transformed.contains("module.exports.bar"),
        "Should export bar"
    );
    assert!(
        transformed.contains("module.exports.Baz"),
        "Should export Baz"
    );
    assert!(
        transformed.contains("module.exports.default"),
        "Should export default"
    );
    assert!(
        transformed.contains("module.exports.x"),
        "Should export x"
    );
    assert!(
        transformed.contains("module.exports.z"),
        "Should export z (aliased from y)"
    );
}

#[test]
fn test_commonjs_detection_module_exports() {
    // CommonJS pattern: module.exports = ...
    let code = "module.exports = { foo: 1 };";
    let transformed = transform_module_code(code);

    // CommonJS code should pass through unchanged
    assert!(
        transformed.contains("module.exports"),
        "Should preserve module.exports"
    );
}

#[test]
fn test_commonjs_detection_exports_dot() {
    // CommonJS pattern: exports.name = ...
    let code = "exports.foo = function() {};\nexports.bar = 42;";
    let transformed = transform_module_code(code);

    // CommonJS code should pass through unchanged
    assert!(transformed.contains("exports.foo"), "Should preserve exports.foo");
    assert!(transformed.contains("exports.bar"), "Should preserve exports.bar");
}
