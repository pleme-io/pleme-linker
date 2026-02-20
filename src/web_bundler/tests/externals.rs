//! Externals configuration tests
//!
//! Tests for configurable external modules and CDN loading.
//! Covers ExternalsMap, base module extraction, and default configurations.

use super::*;
use crate::web_bundler::{default_react_externals, get_base_module, ExternalModule, ExternalsMap};

// =====================================================
// EXTERNALS CONFIGURATION TESTS
// Tests for configurable external modules and CDN loading
// =====================================================

#[test]
fn test_get_base_module_simple() {
    assert_eq!(get_base_module("react"), "react");
    assert_eq!(get_base_module("lodash"), "lodash");
}

#[test]
fn test_get_base_module_with_deep_import() {
    assert_eq!(get_base_module("react-dom/client"), "react-dom");
    assert_eq!(get_base_module("lodash/debounce"), "lodash");
}

#[test]
fn test_get_base_module_scoped() {
    assert_eq!(get_base_module("@mui/material"), "@mui/material");
    assert_eq!(get_base_module("@apollo/client"), "@apollo/client");
}

#[test]
fn test_get_base_module_scoped_with_deep_import() {
    assert_eq!(get_base_module("@mui/material/Button"), "@mui/material");
    assert_eq!(get_base_module("@apollo/client/link/http"), "@apollo/client");
}

#[test]
fn test_externals_map_is_external() {
    let externals = vec![
        ExternalModule::new("react", "React", None),
        ExternalModule::new("react-dom", "ReactDOM", None),
    ];
    let map = ExternalsMap::from_externals(&externals);

    assert!(map.is_external("react"), "react should be external");
    assert!(
        map.is_external("react-dom/client"),
        "react-dom/client should be external"
    );
    assert!(
        !map.is_external("lodash"),
        "lodash should not be external"
    );
    assert!(!map.is_external("./App"), "local should not be external");
}

#[test]
fn test_externals_map_get_global() {
    let externals = vec![
        ExternalModule::new("react", "React", None),
        ExternalModule::new("@mui/material", "MaterialUI", None),
    ];
    let map = ExternalsMap::from_externals(&externals);

    assert_eq!(map.get_global("react"), Some("React".to_string()));
    assert_eq!(
        map.get_global("@mui/material"),
        Some("MaterialUI".to_string())
    );
    assert_eq!(
        map.get_global("@mui/material/Button"),
        Some("MaterialUI".to_string())
    );
    assert_eq!(map.get_global("lodash"), None);
}

#[test]
fn test_externals_map_cdn_scripts() {
    let externals = vec![
        ExternalModule::new(
            "react",
            "React",
            Some("https://unpkg.com/react@18/umd/react.production.min.js"),
        ),
        ExternalModule::new(
            "react-dom",
            "ReactDOM",
            Some("https://unpkg.com/react-dom@18/umd/react-dom.production.min.js"),
        ),
    ];
    let map = ExternalsMap::from_externals(&externals);
    let scripts = map.get_cdn_scripts();

    assert!(
        scripts.contains("https://unpkg.com/react@18"),
        "Should contain React CDN"
    );
    assert!(
        scripts.contains("https://unpkg.com/react-dom@18"),
        "Should contain ReactDOM CDN"
    );
    assert!(scripts.contains("<script"), "Should have script tags");
}

#[test]
fn test_externals_map_empty_cdn() {
    let externals = vec![ExternalModule::new("react", "React", None)];
    let map = ExternalsMap::from_externals(&externals);
    let scripts = map.get_cdn_scripts();

    assert!(scripts.is_empty(), "Should be empty when no CDN URLs");
}

#[test]
fn test_default_react_externals() {
    let externals = default_react_externals();

    // Check that default externals include React ecosystem
    let packages: Vec<&str> = externals.iter().map(|e| e.package.as_str()).collect();
    assert!(packages.contains(&"react"), "Should include react");
    assert!(packages.contains(&"react-dom"), "Should include react-dom");
    assert!(
        packages.contains(&"@mui/material"),
        "Should include @mui/material"
    );
    assert!(
        packages.contains(&"@apollo/client"),
        "Should include @apollo/client"
    );

    // Check that React and ReactDOM have CDN URLs
    let react = externals.iter().find(|e| e.package == "react").unwrap();
    assert!(react.cdn_url.is_some(), "React should have CDN URL");
}

#[test]
fn test_external_module_new() {
    let ext = ExternalModule::new("test-pkg", "TestPkg", Some("https://cdn.example.com/test.js"));

    assert_eq!(ext.package, "test-pkg");
    assert_eq!(ext.global, "TestPkg");
    assert_eq!(
        ext.cdn_url,
        Some("https://cdn.example.com/test.js".to_string())
    );
}

#[test]
fn test_web_bundle_config_default() {
    let config = WebBundleConfig::default();

    assert_eq!(config.base_path, "/");
    assert!(!config.minify);
    assert!(!config.bundle_node_modules);
    assert!(
        !config.externals.is_empty(),
        "Should have default externals"
    );
}
