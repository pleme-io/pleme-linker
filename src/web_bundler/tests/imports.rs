//! Import transformation tests
//!
//! Tests for converting ES module imports to CommonJS require() calls
//! and mapping external modules to window globals.

use super::transform_module_code;

// =====================================================
// Basic Import Tests
// =====================================================

#[test]
fn test_import_default() {
    let code = r#"import React from 'react';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("window.React"),
        "Should use global for external: got '{}'",
        transformed
    );
    assert!(
        !transformed.contains("import"),
        "Should not contain import: got '{}'",
        transformed
    );
}

#[test]
fn test_import_default_local() {
    let code = r#"import App from './App';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("require('./App')"),
        "Should use require for local: got '{}'",
        transformed
    );
    assert!(
        !transformed.contains("import"),
        "Should not contain import: got '{}'",
        transformed
    );
}

#[test]
fn test_import_named() {
    let code = r#"import { useState, useEffect } from 'react';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("window.React"),
        "Should use global for React: got '{}'",
        transformed
    );
    assert!(
        transformed.contains("useState"),
        "Should keep named imports: got '{}'",
        transformed
    );
}

#[test]
fn test_import_named_with_alias() {
    let code = r#"import { foo as bar } from './utils';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("foo: bar"),
        "Should transform 'as' to destructuring: got '{}'",
        transformed
    );
}

#[test]
fn test_import_star() {
    let code = r#"import * as Utils from './utils';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("var Utils = require('./utils')"),
        "Should import star: got '{}'",
        transformed
    );
}

#[test]
fn test_import_mixed() {
    let code = r#"import React, { useState } from 'react';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("window.React"),
        "Should use global: got '{}'",
        transformed
    );
    assert!(
        transformed.contains("useState"),
        "Should keep named: got '{}'",
        transformed
    );
}

// =====================================================
// Comprehensive Import Tests (MDN Full Coverage)
// =====================================================

#[test]
fn test_import_single_named() {
    let code = r#"import { useState } from 'react';"#;
    let transformed = transform_module_code(code);
    assert!(transformed.contains("useState"), "Should have useState");
    assert!(!transformed.contains("import"), "Should not contain import");
}

#[test]
fn test_import_multiple_named() {
    let code = r#"import { useState, useEffect, useCallback, useMemo } from 'react';"#;
    let transformed = transform_module_code(code);
    assert!(transformed.contains("useState"), "Should have useState");
    assert!(transformed.contains("useEffect"), "Should have useEffect");
    assert!(transformed.contains("useCallback"), "Should have useCallback");
    assert!(transformed.contains("useMemo"), "Should have useMemo");
}

#[test]
fn test_import_multiple_aliases() {
    let code = r#"import { foo as a, bar as b, baz as c } from './utils';"#;
    let transformed = transform_module_code(code);
    assert!(transformed.contains("foo: a"), "Should alias foo as a");
    assert!(transformed.contains("bar: b"), "Should alias bar as b");
    assert!(transformed.contains("baz: c"), "Should alias baz as c");
}

#[test]
fn test_import_default_with_namespace() {
    let code = r#"import React, * as ReactAll from 'react';"#;
    let transformed = transform_module_code(code);
    assert!(transformed.contains("React"), "Should have React");
    assert!(!transformed.contains("import"), "Should not contain import");
}

#[test]
fn test_import_default_as_named() {
    let code = r#"import { default as MyComponent } from './Component';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("require('./Component')"),
        "Should require module"
    );
    assert!(!transformed.contains("import"), "Should not contain import");
}

#[test]
fn test_import_side_effect_local() {
    let code = r#"import './styles.css';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("require('./styles.css')"),
        "Should require local: got '{}'",
        transformed
    );
}

#[test]
fn test_import_side_effect_external() {
    let code = r#"import 'normalize.css';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("external side-effect"),
        "Should mark as external: got '{}'",
        transformed
    );
}

#[test]
fn test_import_deep_path_external() {
    let code = r#"import { createRoot } from 'react-dom/client';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("ReactDOM"),
        "Should use ReactDOM global: got '{}'",
        transformed
    );
    assert!(transformed.contains("createRoot"), "Should have createRoot");
}

#[test]
fn test_import_scoped_package() {
    let code = r#"import { Button } from '@mui/material';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("MaterialUI"),
        "Should use MaterialUI global: got '{}'",
        transformed
    );
}

#[test]
fn test_import_scoped_package_deep() {
    let code = r#"import Add from '@mui/icons-material/Add';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("MaterialIcons"),
        "Should use MaterialIcons global: got '{}'",
        transformed
    );
}

#[test]
fn test_import_path_alias() {
    let code = r#"import { utils } from '@/lib/utils';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("require('@/lib/utils')"),
        "Should treat @/ as local: got '{}'",
        transformed
    );
}

#[test]
fn test_import_relative_parent() {
    let code = r#"import { helper } from '../utils/helper';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("require('../utils/helper')"),
        "Should use require for parent path"
    );
}

#[test]
fn test_import_absolute_path() {
    let code = r#"import { config } from '/config/app';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("require('/config/app')"),
        "Should use require for absolute path"
    );
}

#[test]
fn test_import_without_semicolon() {
    let code = "import React from 'react'";
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("window.React"),
        "Should work without semicolon: got '{}'",
        transformed
    );
}

#[test]
fn test_whitespace_variations_in_import() {
    let code1 = "import{useState}from'react';";
    let code2 = "import  {  useState  }  from  'react'  ;";
    let code3 = "import {\n  useState\n} from 'react';";

    let t1 = transform_module_code(code1);
    let t2 = transform_module_code(code2);
    let t3 = transform_module_code(code3);

    assert!(t1.contains("useState"), "No spaces: got '{}'", t1);
    assert!(t2.contains("useState"), "Extra spaces: got '{}'", t2);
    assert!(t3.contains("useState"), "Newlines: got '{}'", t3);
}

#[test]
fn test_multiline_import() {
    let code = r#"import {
    useState,
    useEffect,
    useCallback
} from 'react';"#;
    let transformed = transform_module_code(code);
    assert!(transformed.contains("useState"), "Should have useState");
    assert!(transformed.contains("useEffect"), "Should have useEffect");
    assert!(transformed.contains("useCallback"), "Should have useCallback");
    assert!(!transformed.contains("import"), "Should not contain import");
}

#[test]
fn test_multiple_import_statements() {
    let code = r#"import React from 'react';
import { useState } from 'react';
import App from './App';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("window.React"),
        "Should have React global"
    );
    assert!(transformed.contains("useState"), "Should have useState");
    assert!(transformed.contains("require('./App')"), "Should require App");
    assert!(
        !transformed.contains("import "),
        "Should not contain import"
    );
}

#[test]
fn test_very_long_import() {
    let code = r#"import { ComponentA, ComponentB, ComponentC, ComponentD, ComponentE, ComponentF, ComponentG, ComponentH } from './components';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("ComponentA"),
        "Should have ComponentA"
    );
    assert!(
        transformed.contains("ComponentH"),
        "Should have ComponentH"
    );
    assert!(!transformed.contains("import"), "Should not contain import");
}

#[test]
fn test_import_json() {
    let code = r#"import data from './data.json';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("require('./data.json')"),
        "Should require json: got '{}'",
        transformed
    );
}

#[test]
fn test_import_css() {
    let code = r#"import './global.css';
import styles from './styles.module.css';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("require('./global.css')"),
        "Should require global css"
    );
    assert!(
        transformed.contains("require('./styles.module.css')"),
        "Should require module css"
    );
}

#[test]
fn test_import_from_nested_package() {
    let code = r#"import { something } from '@scope/package/deeply/nested/module';"#;
    let transformed = transform_module_code(code);
    assert!(
        !transformed.contains("require('@scope/package/deeply"),
        "Should use global: got '{}'",
        transformed
    );
}

#[test]
fn test_all_external_imports() {
    let code = r#"import React from 'react';
import ReactDOM from 'react-dom';
import { BrowserRouter } from 'react-router-dom';
import { ThemeProvider } from '@mui/material';
import { ApolloProvider } from '@apollo/client';
import { QueryClient } from '@tanstack/react-query';"#;
    let transformed = transform_module_code(code);
    assert!(
        transformed.contains("window.React"),
        "Should have React global"
    );
    assert!(
        transformed.contains("window.ReactDOM"),
        "Should have ReactDOM global"
    );
    assert!(
        transformed.contains("window.ReactRouterDOM"),
        "Should have ReactRouterDOM global"
    );
    assert!(
        transformed.contains("window.MaterialUI"),
        "Should have MUI global"
    );
    assert!(
        transformed.contains("window.Apollo"),
        "Should have Apollo global"
    );
    assert!(
        transformed.contains("window.ReactQuery"),
        "Should have ReactQuery global"
    );
    assert!(
        !transformed.contains("import "),
        "Should not contain any import statements"
    );
}

#[test]
fn test_deeply_nested_local_imports() {
    let code = r#"import { a } from './a';
import { b } from '../b';
import { c } from '../../c';
import { d } from '../../../d';
import { e } from '../../../../e';"#;
    let transformed = transform_module_code(code);
    assert!(transformed.contains("require('./a')"), "Should require ./a");
    assert!(transformed.contains("require('../b')"), "Should require ../b");
    assert!(
        transformed.contains("require('../../c')"),
        "Should require ../../c"
    );
    assert!(
        transformed.contains("require('../../../d')"),
        "Should require ../../../d"
    );
    assert!(
        transformed.contains("require('../../../../e')"),
        "Should require ../../../../e"
    );
}
