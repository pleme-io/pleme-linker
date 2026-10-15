//! import.meta transformation tests
//!
//! Tests for Vite-compatible import.meta transformations.
//! Vite uses import.meta.env for environment variables.

use super::transform_module_code;

// =====================================================
// IMPORT.META TRANSFORMATION TESTS
// Vite uses import.meta.env for environment variables
// =====================================================

#[test]
fn test_import_meta_env_dev() {
    let code = "const isDev = import.meta.env.DEV;";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("false"),
        "import.meta.env.DEV should become false: got '{}'",
        transformed
    );
    assert!(
        !transformed.contains("import.meta"),
        "Should not contain import.meta: got '{}'",
        transformed
    );
}

#[test]
fn test_import_meta_env_prod() {
    let code = "const isProd = import.meta.env.PROD;";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("true"),
        "import.meta.env.PROD should become true: got '{}'",
        transformed
    );
}

#[test]
fn test_import_meta_env_mode() {
    let code = "const mode = import.meta.env.MODE;";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("'production'"),
        "import.meta.env.MODE should become 'production': got '{}'",
        transformed
    );
}

#[test]
fn test_import_meta_env_base_url() {
    let code = "const base = import.meta.env.BASE_URL;";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("'/'"),
        "import.meta.env.BASE_URL should become '/': got '{}'",
        transformed
    );
}

#[test]
fn test_import_meta_env_custom_var() {
    let code = "const apiUrl = import.meta.env.VITE_API_URL;";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("window.ENV"),
        "Custom env vars should use window.ENV: got '{}'",
        transformed
    );
    assert!(
        transformed.contains("VITE_API_URL"),
        "Should reference the variable name: got '{}'",
        transformed
    );
}

#[test]
fn test_import_meta_env_object() {
    let code = "const env = import.meta.env;";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("window.ENV"),
        "import.meta.env should use window.ENV: got '{}'",
        transformed
    );
}

#[test]
fn test_import_meta_url() {
    let code = "const url = import.meta.url;";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("window.location.href"),
        "import.meta.url should become window.location.href: got '{}'",
        transformed
    );
}

#[test]
fn test_import_meta_env_ssr() {
    let code = "if (import.meta.env.SSR) { serverRender(); }";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("false"),
        "import.meta.env.SSR should become false: got '{}'",
        transformed
    );
}

#[test]
fn test_import_meta_multiple_in_same_file() {
    let code = r#"
const isDev = import.meta.env.DEV;
const apiUrl = import.meta.env.VITE_API_URL;
const mode = import.meta.env.MODE;
"#;
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("import.meta"),
        "Should not contain any import.meta: got '{}'",
        transformed
    );
    assert!(transformed.contains("false"), "Should have DEV=false");
    assert!(
        transformed.contains("'production'"),
        "Should have MODE='production'"
    );
    assert!(
        transformed.contains("window.ENV"),
        "Should have window.ENV for custom var"
    );
}
