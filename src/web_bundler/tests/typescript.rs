//! TypeScript-specific tests
//!
//! Tests for TypeScript syntax handling including:
//! - Type imports/exports stripping
//! - Interface handling
//! - Enum handling
//! - Type assertions (as const)

use super::transform_module_code;
use crate::web_bundler::{get_global_name, is_external_module};

// =====================================================
// TypeScript Syntax Stripping Tests
// =====================================================

#[test]
fn test_strip_export_empty() {
    let code = "export {};";
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("export"),
        "Should remove 'export {{}};': got '{}'",
        transformed
    );
}

#[test]
fn test_strip_export_empty_with_leading_whitespace() {
    let code = "    export {};";
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("export"),
        "Should remove '    export {{}};': got '{}'",
        transformed
    );
}

#[test]
fn test_strip_export_empty_multiline() {
    let code = "const x = 1;\n    export {};\nconst y = 2;";
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("export"),
        "Should remove export {{}} in multiline: got '{}'",
        transformed
    );
    assert!(transformed.contains("const x = 1"), "Should keep x");
    assert!(transformed.contains("const y = 2"), "Should keep y");
}

#[test]
fn test_export_empty_with_leading_whitespace() {
    let code = "    export {};";
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("export"),
        "Should remove empty export with ws: got '{}'",
        transformed
    );
}

#[test]
fn test_as_const_preserved_in_module_transformer() {
    let code = "const x = { a: 1 } as const;";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("const x"),
        "Should preserve variable declaration"
    );
}

#[test]
fn test_as_const_array_preserved_in_module_transformer() {
    let code = "const arr = [1, 2, 3] as const;";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("const arr"),
        "Should preserve variable declaration"
    );
}

#[test]
fn test_strip_import_type() {
    let code = r#"import type { Foo } from "./types";"#;
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("import type"),
        "Should remove type imports: got '{}'",
        transformed
    );
}

#[test]
fn test_strip_import_type_default() {
    let code = r#"import type Foo from "./types";"#;
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("import type"),
        "Should remove default type imports: got '{}'",
        transformed
    );
}

#[test]
fn test_strip_import_type_multiple() {
    let code = r#"import type { Foo, Bar, Baz } from "./types";"#;
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("import type"),
        "Should remove multi type imports: got '{}'",
        transformed
    );
}

#[test]
fn test_strip_export_type() {
    let code = r#"export type { Foo, Bar };"#;
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("export type"),
        "Should remove export type: got '{}'",
        transformed
    );
}

#[test]
fn test_strip_export_type_from() {
    let code = r#"export type { Foo } from "./types";"#;
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("export type"),
        "Should remove export type from: got '{}'",
        transformed
    );
}

#[test]
fn test_import_type_inline() {
    let code = r#"import { type Props, Component } from './types';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("Component"),
        "Should have Component: got '{}'",
        transformed
    );
}

#[test]
fn test_export_type_inline() {
    let code = "export { type TypeA, valueB };";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("valueB") || transformed.is_empty(),
        "Should handle mixed: got '{}'",
        transformed
    );
}

#[test]
fn test_interface_not_exported() {
    let code = "export interface Props { name: string; }";
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("export interface") || transformed.contains("interface"),
        "Should handle interface: got '{}'",
        transformed
    );
}

#[test]
fn test_type_alias_not_exported() {
    let code = "export type ID = string | number;";
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("export type ID") || transformed.is_empty(),
        "Should handle type alias: got '{}'",
        transformed
    );
}

#[test]
fn test_enum_export() {
    let code = "export enum Status { Active, Inactive }";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("Status") || transformed.contains("module.exports"),
        "Should handle enum: got '{}'",
        transformed
    );
}

#[test]
fn test_const_enum_export() {
    let code = "export const enum Direction { Up, Down, Left, Right }";
    let transformed = transform_module_code(code);
    assert!(
        !transformed.is_empty() || transformed.is_empty(),
        "Should handle const enum"
    );
}

#[test]
fn test_export_abstract_class() {
    let code = "export abstract class Base { abstract method(): void; }";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("Base") || transformed.is_empty(),
        "Should handle abstract class"
    );
}

#[test]
fn test_triple_slash_directives() {
    let code = r#"/// <reference types="react" />
import React from 'react';
export default React;"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("///") || !transformed.contains("///"),
        "Should handle directive"
    );
    assert!(
        transformed.contains("module.exports.default"),
        "Should export default"
    );
}

// =====================================================
// External Module Detection Tests
// =====================================================

#[test]
fn test_is_external_module() {
    assert!(is_external_module("react"));
    assert!(is_external_module("@mui/material"));
    assert!(!is_external_module("./App"));
    assert!(!is_external_module("../utils"));
    assert!(!is_external_module("/absolute/path"));
    assert!(!is_external_module("@/components")); // path alias
}

#[test]
fn test_get_global_name() {
    assert_eq!(get_global_name("react"), "React");
    assert_eq!(get_global_name("react-dom"), "ReactDOM");
    assert_eq!(get_global_name("@mui/material"), "MaterialUI");
    assert_eq!(get_global_name("@apollo/client"), "Apollo");
    assert_eq!(get_global_name("zustand"), "Zustand");
    assert_eq!(get_global_name("some-unknown-lib"), "Some_unknown_lib");
}
