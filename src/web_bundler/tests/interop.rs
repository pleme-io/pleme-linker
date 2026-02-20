//! CommonJS/ESM interop tests
//!
//! Tests for proper interoperability between CommonJS and ESM modules.
//! This includes __esModule marker, default export handling, and mixed imports.

use super::transform_module_code;

// =====================================================
// Default Export Interop Tests
// =====================================================

#[test]
fn test_esm_default_export_to_commonjs() {
    // ESM: export default X
    // Should be accessible as require().default or require() directly
    let code = "export default function App() { return null; }";
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("module.exports.default"),
        "Should set default export: got '{}'",
        transformed
    );
}

#[test]
fn test_esm_named_and_default_export() {
    // ESM with both default and named exports
    let code = r#"
export const version = "1.0";
export default class Component {}
"#;
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("module.exports.version"),
        "Should export version"
    );
    assert!(
        transformed.contains("module.exports.default"),
        "Should export default"
    );
}

#[test]
fn test_import_default_from_commonjs_like() {
    // When importing default from a CommonJS module, should handle both:
    // - module.exports = X (default is the whole export)
    // - module.exports.default = X (explicit default)
    let code = r#"import App from './App';"#;
    let transformed = transform_module_code(code);

    // Our transform uses: var App = mod.default || mod;
    // This handles both cases
    assert!(
        transformed.contains(".default ||"),
        "Should handle CJS default fallback: got '{}'",
        transformed
    );
}

#[test]
fn test_import_named_from_module() {
    // Named imports should work regardless of module type
    let code = r#"import { useState, useEffect } from 'react';"#;
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("useState"),
        "Should destructure useState"
    );
    assert!(
        transformed.contains("useEffect"),
        "Should destructure useEffect"
    );
}

// =====================================================
// Mixed Module System Tests
// =====================================================

#[test]
fn test_esm_reexport_from_commonjs() {
    // Re-exporting from a CJS module
    let code = r#"export { default as utils } from './utils';"#;
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("module.exports.utils"),
        "Should re-export as utils: got '{}'",
        transformed
    );
    assert!(
        transformed.contains("require('./utils')"),
        "Should require the module"
    );
}

#[test]
fn test_namespace_import_interop() {
    // import * as X handles both ESM and CJS
    let code = r#"import * as Utils from './utils';"#;
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("var Utils = require('./utils')"),
        "Should create namespace: got '{}'",
        transformed
    );
}

#[test]
fn test_export_star_interop() {
    // export * should work with both ESM and CJS source
    let code = r#"export * from './helpers';"#;
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("Object.assign(module.exports, require('./helpers'))"),
        "Should spread all exports: got '{}'",
        transformed
    );
}

// =====================================================
// Edge Cases for Interop
// =====================================================

#[test]
fn test_default_import_alias() {
    // import { default as X } is equivalent to import X
    let code = r#"import { default as MyApp } from './App';"#;
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("require('./App')"),
        "Should require module"
    );
    // The default should be properly extracted
}

#[test]
fn test_reexport_default_as_default() {
    // export { default } from './module'
    let code = r#"export { default } from './Component';"#;
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("module.exports.default"),
        "Should re-export default: got '{}'",
        transformed
    );
}

#[test]
fn test_mixed_import_default_and_named() {
    // import Default, { named } from 'module'
    let code = r#"import React, { useState, useEffect } from 'react';"#;
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("window.React"),
        "Should use React global"
    );
    assert!(transformed.contains("useState"), "Should have useState");
    assert!(transformed.contains("useEffect"), "Should have useEffect");
}

#[test]
fn test_dynamic_import_preserved() {
    // Dynamic imports should be preserved (they're async)
    let code = r#"const mod = await import('./dynamic.js');"#;
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("import('./dynamic.js')"),
        "Should preserve dynamic import: got '{}'",
        transformed
    );
}

// =====================================================
// External Module Interop
// =====================================================

#[test]
fn test_external_default_import() {
    // External modules are accessed via window globals
    let code = r#"import ReactDOM from 'react-dom';"#;
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("window.ReactDOM"),
        "Should use window global: got '{}'",
        transformed
    );
    assert!(
        transformed.contains(".default ||"),
        "Should handle default fallback"
    );
}

#[test]
fn test_external_namespace_import() {
    // import * as X from 'external'
    let code = r#"import * as React from 'react';"#;
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("window.React"),
        "Should use React global: got '{}'",
        transformed
    );
}

#[test]
fn test_external_deep_import() {
    // Deep imports from external modules
    let code = r#"import { createRoot } from 'react-dom/client';"#;
    let transformed = transform_module_code(code);

    assert!(
        transformed.contains("ReactDOM"),
        "Should use ReactDOM global for react-dom/client"
    );
    assert!(transformed.contains("createRoot"), "Should extract createRoot");
}
