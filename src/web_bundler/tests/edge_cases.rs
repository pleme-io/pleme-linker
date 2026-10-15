//! Edge cases and real-world pattern tests
//!
//! Tests covering complex scenarios, real-world patterns,
//! and edge cases for production reliability.

use super::transform_module_code;

// =====================================================
// EDGE CASES AND COMPLEX SCENARIOS
// =====================================================

#[test]
fn test_import_in_string_literal_not_transformed() {
    let code = r#"const str = "import React from 'react';";"#;
    let transformed = transform_module_code(code);
    // String literals should NOT be transformed
    assert!(transformed.contains("const str"), "Should keep variable");
    // The string content should be preserved
    assert!(
        transformed.contains("import React"),
        "Should preserve string content: got '{}'",
        transformed
    );
}

#[test]
fn test_export_in_string_literal_not_transformed() {
    let code = r#"const str = "export default App;";"#;
    let transformed = transform_module_code(code);
    assert!(transformed.contains("const str"), "Should keep variable");
    assert!(
        transformed.contains("export default"),
        "Should preserve string content: got '{}'",
        transformed
    );
}

#[test]
fn test_mixed_imports_and_exports() {
    // Note: Using plain JS, not JSX, since JSX is compiled in the earlier stage
    let code = r#"import React from 'react';
import { useState } from 'react';

const MyComponent = () => React.createElement('div', null);

export default MyComponent;
export { useState };"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("window.React"),
        "Should have React: got '{}'",
        transformed
    );
    assert!(
        transformed.contains("module.exports.default"),
        "Should export default"
    );
    assert!(
        transformed.contains("module.exports.useState"),
        "Should export useState"
    );
    assert!(
        !transformed.contains("import "),
        "Should not contain import"
    );
    assert!(
        !transformed.contains("export default"),
        "Should not contain export default"
    );
}

#[test]
fn test_empty_file() {
    let code = "";
    let transformed = transform_module_code(code);
    assert_eq!(transformed, "", "Empty file should remain empty");
}

#[test]
fn test_only_code_no_modules() {
    let code = "const x = 1;\nconst y = 2;\nconsole.log(x + y);";
    let transformed = transform_module_code(code);
    assert_eq!(
        transformed, code,
        "Code without modules should be unchanged"
    );
}

#[test]
fn test_comments_preserved() {
    let code = r#"// This is a comment
import React from 'react';
/* Another comment */
const App = () => <div />;
export default App;"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("// This is a comment"),
        "Should preserve single-line comment"
    );
    assert!(
        transformed.contains("/* Another comment */"),
        "Should preserve multi-line comment"
    );
}

#[test]
fn test_dynamic_import_not_transformed() {
    // Dynamic imports should NOT be transformed to require (they're async)
    let code = r#"const module = await import('./dynamic');"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("import('./dynamic')"),
        "Should preserve dynamic import: got '{}'",
        transformed
    );
}

#[test]
fn test_export_from_index_barrel() {
    // Common pattern: barrel file re-exporting everything
    let code = r#"export { Button } from './Button';
export { Input } from './Input';
export { Modal } from './Modal';
export * from './utils';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.Button"),
        "Should export Button"
    );
    assert!(
        transformed.contains("module.exports.Input"),
        "Should export Input"
    );
    assert!(
        transformed.contains("module.exports.Modal"),
        "Should export Modal"
    );
    assert!(
        transformed.contains("Object.assign"),
        "Should have star re-export"
    );
}

#[test]
fn test_export_destructured_const() {
    // export const { a, b } = obj;
    let code = "const obj = { a: 1, b: 2 };\nexport const { a, b } = obj;";
    let transformed = transform_module_code(code);
    // Destructuring exports are complex - verify it doesn't crash
    assert!(transformed.contains("const obj"), "Should preserve obj");
}

#[test]
fn test_jsx_compiled_preserved() {
    // JSX is compiled by OXC to createElement calls before module transformation
    let code = r#"import React from 'react';
const App = () => React.createElement("div", { className: "app" }, React.createElement("span", null, "Hello"));
export default App;"#;
    let transformed = transform_module_code(code);
    assert!(transformed.contains("App"), "Should have App");
    assert!(
        transformed.contains("createElement"),
        "Should preserve createElement calls"
    );
    assert!(
        transformed.contains("module.exports.default"),
        "Should export default"
    );
}

#[test]
fn test_template_literal_with_import_text() {
    let code = "const code = `import React from 'react';`;";
    let transformed = transform_module_code(code);
    assert!(transformed.contains("const code"), "Should preserve variable");
    // Template literal content should be preserved
    assert!(
        transformed.contains("`import React"),
        "Should preserve template literal: got '{}'",
        transformed
    );
}

#[test]
fn test_export_with_semicolon_variations() {
    // Some code has semicolons, some doesn't
    let code1 = "export default App;";
    let code2 = "export default App";
    let t1 = transform_module_code(code1);
    let t2 = transform_module_code(code2);
    assert!(
        t1.contains("module.exports.default"),
        "With semicolon: got '{}'",
        t1
    );
    assert!(
        t2.contains("module.exports.default"),
        "Without semicolon: got '{}'",
        t2
    );
}

#[test]
fn test_unicode_in_module_path() {
    let code = r#"import data from './données';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("require('./données')"),
        "Should handle unicode: got '{}'",
        transformed
    );
}

#[test]
fn test_special_chars_in_export_name() {
    // Valid JS allows some special characters in identifiers
    let code = "export const $special = 1;\nexport const _underscore = 2;";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.$special"),
        "Should export $special"
    );
    assert!(
        transformed.contains("module.exports._underscore"),
        "Should export _underscore"
    );
}

// =====================================================
// REAL-WORLD PATTERNS
// =====================================================

#[test]
fn test_react_component_pattern() {
    // Note: Using compiled JSX (createElement), not raw JSX
    // In the actual pipeline, OXC compiles JSX before module transformation
    let code = r#"import React, { useState, useEffect } from 'react';
import { Button } from '@mui/material';
import './styles.css';

interface Props {
    title: string;
}

const MyComponent: React.FC<Props> = ({ title }) => {
    const [count, setCount] = useState(0);

    useEffect(() => {
        document.title = title;
    }, [title]);

    return React.createElement(Button, { onClick: () => setCount(c => c + 1) }, count);
};

export default MyComponent;
export { MyComponent };"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("window.React"),
        "Should have React: got '{}'",
        transformed
    );
    assert!(transformed.contains("MaterialUI"), "Should have MUI");
    assert!(
        transformed.contains("module.exports.default"),
        "Should export default"
    );
    assert!(
        transformed.contains("module.exports.MyComponent"),
        "Should export named"
    );
    assert!(
        !transformed.contains("import "),
        "Should not contain import statements"
    );
}

#[test]
fn test_service_module_pattern() {
    let code = r#"import { apiClient } from './apiClient';
import type { User, Response } from './types';

export async function getUser(id: string): Promise<User> {
    return apiClient.get(`/users/${id}`);
}

export async function updateUser(id: string, data: Partial<User>): Promise<User> {
    return apiClient.put(`/users/${id}`, data);
}

export const userService = {
    getUser,
    updateUser,
};"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("require('./apiClient')"),
        "Should require apiClient"
    );
    assert!(
        transformed.contains("module.exports.getUser"),
        "Should export getUser"
    );
    assert!(
        transformed.contains("module.exports.updateUser"),
        "Should export updateUser"
    );
    assert!(
        transformed.contains("module.exports.userService"),
        "Should export userService"
    );
}

#[test]
fn test_index_barrel_pattern() {
    let code = r#"export * from './Button';
export * from './Input';
export * from './Modal';
export { default as Layout } from './Layout';
export type { ButtonProps, InputProps, ModalProps } from './types';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("Object.assign(module.exports, require('./Button'))"),
        "Should re-export Button"
    );
    assert!(
        transformed.contains("Object.assign(module.exports, require('./Input'))"),
        "Should re-export Input"
    );
    assert!(
        transformed.contains("module.exports.Layout"),
        "Should export Layout"
    );
    // Type exports should be stripped
    assert!(
        !transformed.contains("ButtonProps"),
        "Should strip type exports: got '{}'",
        transformed
    );
}

// =====================================================
// ADDITIONAL EDGE CASES FOR PRODUCTION RELIABILITY
// =====================================================

#[test]
fn test_export_class_with_static_methods() {
    let code = r#"export class Utils {
    static format(str) { return str.trim(); }
    static parse(json) { return JSON.parse(json); }
}"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.Utils"),
        "Should export Utils"
    );
    assert!(
        transformed.contains("static format"),
        "Should preserve static methods"
    );
}

#[test]
fn test_export_arrow_variations() {
    let code = r#"export const fn1 = () => 1;
export const fn2 = (x) => x * 2;
export const fn3 = (x, y) => x + y;
export const fn4 = x => x;
export const fn5 = async () => await fetch('/');
export const fn6 = async (url) => { return await fetch(url); };"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.fn1"),
        "Should export fn1"
    );
    assert!(
        transformed.contains("module.exports.fn2"),
        "Should export fn2"
    );
    assert!(
        transformed.contains("module.exports.fn3"),
        "Should export fn3"
    );
    assert!(
        transformed.contains("module.exports.fn4"),
        "Should export fn4"
    );
    assert!(
        transformed.contains("module.exports.fn5"),
        "Should export fn5"
    );
    assert!(
        transformed.contains("module.exports.fn6"),
        "Should export fn6"
    );
}

#[test]
fn test_export_object_shorthand() {
    let code = r#"const a = 1;
const b = 2;
export { a, b };"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.a = a"),
        "Should export a"
    );
    assert!(
        transformed.contains("module.exports.b = b"),
        "Should export b"
    );
}

#[test]
fn test_reexport_everything_pattern() {
    // Common in index.ts files
    let code = r#"export * from './a';
export * from './b';
export * from './c';
export * from './d';
export * from './e';"#;
    let transformed = transform_module_code(code);
    assert_eq!(
        transformed.matches("Object.assign").count(),
        5,
        "Should have 5 re-exports"
    );
}

#[test]
fn test_mixed_default_and_named_reexport() {
    let code = r#"export { default, foo, bar } from './module';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default"),
        "Should re-export default"
    );
    assert!(
        transformed.contains("module.exports.foo"),
        "Should re-export foo"
    );
    assert!(
        transformed.contains("module.exports.bar"),
        "Should re-export bar"
    );
}

#[test]
fn test_chained_exports() {
    let code = r#"const x = 1;
const y = 2;
const z = 3;
export { x };
export { y };
export { z };"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.x = x"),
        "Should export x"
    );
    assert!(
        transformed.contains("module.exports.y = y"),
        "Should export y"
    );
    assert!(
        transformed.contains("module.exports.z = z"),
        "Should export z"
    );
}

#[test]
fn test_export_hoisted_function() {
    // Functions are hoisted, so export can come before declaration
    let code = r#"export { greet };
function greet() { return 'Hello'; }"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.greet"),
        "Should export greet"
    );
    assert!(
        transformed.contains("function greet"),
        "Should preserve function"
    );
}

#[test]
fn test_export_default_class_with_constructor() {
    let code = r#"export default class MyClass {
    constructor(name) {
        this.name = name;
    }

    greet() {
        return `Hello, ${this.name}`;
    }
}"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default"),
        "Should export class"
    );
    assert!(
        transformed.contains("constructor"),
        "Should preserve constructor"
    );
    assert!(transformed.contains("greet"), "Should preserve method");
}

#[test]
fn test_many_named_exports_single_statement() {
    let code = "export { a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p };";
    let transformed = transform_module_code(code);
    assert!(transformed.contains("module.exports.a"), "Should export a");
    assert!(transformed.contains("module.exports.p"), "Should export p");
}

#[test]
fn test_export_reassignment() {
    let code = r#"let value = 1;
export { value };
value = 2;"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("let value = 1"),
        "Should preserve declaration"
    );
    assert!(
        transformed.contains("module.exports.value"),
        "Should export value"
    );
}

#[test]
fn test_circular_like_reexports() {
    // Pattern that might appear in circular dependencies
    let code = r#"export { a } from './module-a';
export { b } from './module-b';
export const c = 'local';"#;
    let transformed = transform_module_code(code);
    assert!(transformed.contains("module.exports.a"), "Should export a");
    assert!(transformed.contains("module.exports.b"), "Should export b");
    assert!(transformed.contains("module.exports.c"), "Should export c");
}

#[test]
fn test_default_export_with_same_named_export() {
    let code = r#"const App = () => null;
export default App;
export { App };"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default"),
        "Should have default export"
    );
    assert!(
        transformed.contains("module.exports.App = App"),
        "Should have named export"
    );
}

#[test]
fn test_empty_module() {
    let code = "// This file intentionally left empty";
    let transformed = transform_module_code(code);
    assert_eq!(
        transformed, code,
        "Comments-only file should be unchanged"
    );
}

#[test]
fn test_shebang_preserved() {
    // Node.js files might have shebangs
    let code = r#"#!/usr/bin/env node
import { run } from './cli';
run();"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("#!/usr/bin/env node"),
        "Should preserve shebang: got '{}'",
        transformed
    );
}
