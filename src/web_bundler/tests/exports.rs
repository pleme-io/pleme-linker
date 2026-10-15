//! Export transformation tests
//!
//! Tests for converting ES module exports to CommonJS module.exports
//! and handling various export patterns.

use super::transform_module_code;

// =====================================================
// Basic Export Tests
// =====================================================

#[test]
fn test_export_default_function() {
    let code = "export default function App() {}";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default"),
        "Should export default: got '{}'",
        transformed
    );
    assert!(
        !transformed.contains("export default"),
        "Should not contain export default: got '{}'",
        transformed
    );
}

#[test]
fn test_export_default_expression() {
    let code = "export default App;";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default = App"),
        "Should export default expression: got '{}'",
        transformed
    );
}

#[test]
fn test_export_named() {
    let code = "export { foo, bar };";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.foo = foo"),
        "Should export foo: got '{}'",
        transformed
    );
    assert!(
        transformed.contains("module.exports.bar = bar"),
        "Should export bar: got '{}'",
        transformed
    );
}

#[test]
fn test_export_named_with_alias() {
    let code = "export { foo as baz };";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.baz = foo"),
        "Should export with alias: got '{}'",
        transformed
    );
}

#[test]
fn test_export_const() {
    let code = "export const FOO = 'bar';";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.FOO"),
        "Should export const: got '{}'",
        transformed
    );
    assert!(
        !transformed.contains("export const"),
        "Should not contain export const: got '{}'",
        transformed
    );
}

#[test]
fn test_export_function() {
    let code = "export function hello() {}";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.hello"),
        "Should export function: got '{}'",
        transformed
    );
}

#[test]
fn test_export_class() {
    let code = "export class MyClass {}";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.MyClass"),
        "Should export class: got '{}'",
        transformed
    );
}

// =====================================================
// Re-export Tests
// =====================================================

#[test]
fn test_export_star_from() {
    let code = r#"export * from "./types";"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("Object.assign(module.exports, require('./types'))"),
        "Should re-export star: got '{}'",
        transformed
    );
    assert!(
        !transformed.contains("export *"),
        "Should not contain export *: got '{}'",
        transformed
    );
}

#[test]
fn test_export_star_from_with_leading_whitespace() {
    let code = "    export * from \"./types\";";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("Object.assign(module.exports, require('./types'))"),
        "Should re-export star with leading ws: got '{}'",
        transformed
    );
    assert!(
        !transformed.contains("export *"),
        "Should not contain export * with ws: got '{}'",
        transformed
    );
}

#[test]
fn test_export_star_as_from() {
    let code = r#"export * as utils from "./utils";"#;
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("export *"),
        "Should not contain export *: got '{}'",
        transformed
    );
    assert!(
        transformed.contains("module.exports.utils"),
        "Should export as utils: got '{}'",
        transformed
    );
}

#[test]
fn test_export_star_as_from_with_leading_whitespace() {
    let code = r#"    export * as addressService from "./address.service";"#;
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("export *"),
        "Should not contain export * with ws: got '{}'",
        transformed
    );
    assert!(
        transformed.contains("module.exports.addressService"),
        "Should export addressService: got '{}'",
        transformed
    );
}

#[test]
fn test_export_named_from() {
    let code = r#"export { foo, bar } from "./utils";"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("require('./utils')"),
        "Should require module: got '{}'",
        transformed
    );
    assert!(
        transformed.contains("module.exports.foo"),
        "Should export foo: got '{}'",
        transformed
    );
    assert!(
        transformed.contains("module.exports.bar"),
        "Should export bar: got '{}'",
        transformed
    );
    assert!(
        !transformed.contains("export {"),
        "Should not contain 'export {{': got '{}'",
        transformed
    );
}

#[test]
fn test_export_named_from_with_alias() {
    let code = r#"export { foo as baz } from "./utils";"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.baz = __reexport.foo"),
        "Should re-export with alias: got '{}'",
        transformed
    );
}

#[test]
fn test_reexport_default() {
    let code = r#"export { default } from './Component';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default"),
        "Should re-export default: got '{}'",
        transformed
    );
    assert!(
        transformed.contains("require('./Component')"),
        "Should require module"
    );
}

#[test]
fn test_reexport_default_as_named() {
    let code = r#"export { default as MyComponent } from './Component';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.MyComponent"),
        "Should export as MyComponent: got '{}'",
        transformed
    );
}

#[test]
fn test_reexport_multiple_with_aliases() {
    let code = r#"export { foo as a, bar as b, default as c } from './utils';"#;
    let transformed = transform_module_code(code);
    assert!(transformed.contains("module.exports.a"), "Should export a");
    assert!(transformed.contains("module.exports.b"), "Should export b");
    assert!(transformed.contains("module.exports.c"), "Should export c");
}

#[test]
fn test_reexport_star_external() {
    let code = r#"export * from 'lodash';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("window.Lodash"),
        "Should use global: got '{}'",
        transformed
    );
}

#[test]
fn test_reexport_star_as_external() {
    let code = r#"export * as _ from 'lodash';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports._"),
        "Should export as _: got '{}'",
        transformed
    );
}

// =====================================================
// Comprehensive Export Tests
// =====================================================

#[test]
fn test_export_let() {
    let code = "export let counter = 0;";
    let transformed = transform_module_code(code);
    assert!(transformed.contains("let counter"), "Should have let");
    assert!(
        transformed.contains("module.exports.counter"),
        "Should export counter"
    );
}

#[test]
fn test_export_var() {
    let code = "export var legacy = true;";
    let transformed = transform_module_code(code);
    assert!(transformed.contains("var legacy"), "Should have var");
    assert!(
        transformed.contains("module.exports.legacy"),
        "Should export legacy"
    );
}

#[test]
fn test_export_multiple_const() {
    let code = "export const A = 1, B = 2, C = 3;";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.A"),
        "Should export A: got '{}'",
        transformed
    );
}

#[test]
fn test_export_async_function() {
    let code = "export async function fetchData() { return await fetch('/api'); }";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.fetchData"),
        "Should export async fn: got '{}'",
        transformed
    );
    assert!(transformed.contains("async function"), "Should preserve async");
}

#[test]
fn test_export_generator_function() {
    let code = "export function* generator() { yield 1; yield 2; }";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.generator"),
        "Should export generator: got '{}'",
        transformed
    );
    assert!(
        transformed.contains("function*"),
        "Should preserve generator syntax"
    );
}

#[test]
fn test_export_default_anonymous_function() {
    let code = "export default function() { return 42; }";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default"),
        "Should export default: got '{}'",
        transformed
    );
}

#[test]
fn test_export_default_anonymous_class() {
    let code = "export default class { constructor() {} }";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default"),
        "Should export default class: got '{}'",
        transformed
    );
}

#[test]
fn test_export_default_async_function() {
    let code = "export default async function loadData() { return await fetch('/'); }";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default"),
        "Should export default async: got '{}'",
        transformed
    );
    assert!(transformed.contains("loadData"), "Should have function name");
}

#[test]
fn test_export_default_arrow_expression() {
    let code = "export default () => 'hello';";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default"),
        "Should export arrow: got '{}'",
        transformed
    );
}

#[test]
fn test_export_default_object() {
    let code = "export default { a: 1, b: 2 };";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default"),
        "Should export object: got '{}'",
        transformed
    );
}

#[test]
fn test_export_default_array() {
    let code = "export default [1, 2, 3];";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default"),
        "Should export array: got '{}'",
        transformed
    );
}

#[test]
fn test_export_default_string() {
    let code = r#"export default "hello world";"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default"),
        "Should export string: got '{}'",
        transformed
    );
}

#[test]
fn test_export_default_number() {
    let code = "export default 42;";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.default = 42"),
        "Should export number: got '{}'",
        transformed
    );
}

#[test]
fn test_export_class_with_extends() {
    let code = "export class Child extends Parent { constructor() { super(); } }";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.Child"),
        "Should export Child: got '{}'",
        transformed
    );
    assert!(transformed.contains("extends Parent"), "Should preserve extends");
}

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
    assert!(transformed.contains("module.exports.fn1"), "Should export fn1");
    assert!(transformed.contains("module.exports.fn2"), "Should export fn2");
    assert!(transformed.contains("module.exports.fn3"), "Should export fn3");
    assert!(transformed.contains("module.exports.fn4"), "Should export fn4");
    assert!(transformed.contains("module.exports.fn5"), "Should export fn5");
    assert!(transformed.contains("module.exports.fn6"), "Should export fn6");
}

#[test]
fn test_export_without_semicolon() {
    let code = "export const x = 1";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("module.exports.x"),
        "Should work without semicolon"
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
fn test_export_from_index_barrel() {
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
    assert!(transformed.contains("Object.assign"), "Should have star re-export");
}

#[test]
fn test_export_destructured_const() {
    let code = "const obj = { a: 1, b: 2 };\nexport const { a, b } = obj;";
    let transformed = transform_module_code(code);
    assert!(transformed.contains("const obj"), "Should preserve obj");
}
