//! Web Application Bundler using OXC
//!
//! Bundles React/TypeScript web applications into production-ready assets:
//! - Module resolution using oxc_resolver
//! - TypeScript/JSX transformation using OXC
//! - AST-based module transformation (ESM → CommonJS)
//! - JavaScript bundling into a single file
//! - index.html generation with script/style tags
//! - Static asset copying with hash-based cache busting
//!
//! This is a pure Rust bundler - no Node.js or Vite required.
//!
//! Architecture:
//! 1. OXC Compiler: TypeScript/TSX → JavaScript (strips types, transforms JSX)
//! 2. OXC Parser: Parse JS into AST for module transformation
//! 3. Module Transformer: ESM (import/export) → CommonJS (require/module.exports)
//! 4. Bundler: Concatenate modules with runtime

use anyhow::{Context, Result};
use oxc::allocator::Allocator;
use oxc::ast::ast::{
    ExportDefaultDeclarationKind, ExportNamedDeclaration, ImportDeclaration,
    ImportDeclarationSpecifier, ModuleExportName, Statement,
};
use oxc::codegen::{CodegenOptions, CodegenReturn};
use oxc::diagnostics::OxcDiagnostic;
use oxc::parser::{ParseOptions, Parser};
use oxc::span::{GetSpan, SourceType};
use oxc::transformer::{JsxOptions, TransformOptions, TypeScriptOptions};
use oxc::CompilerInterface;
use oxc_resolver::{ResolveOptions, Resolver};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::mem;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

/// Custom compiler that enables TypeScript/JSX transforms
/// Configured specifically for React/TypeScript web applications
struct WebCompiler {
    transform_options: TransformOptions,
    printed: String,
    errors: Vec<OxcDiagnostic>,
}

impl WebCompiler {
    fn new() -> Self {
        // Configure TypeScript transform to strip all type information
        let typescript = TypeScriptOptions {
            // Remove all unused imports, not just type-only ones
            only_remove_type_imports: false,
            // Allow TypeScript namespaces
            allow_namespaces: true,
            // Allow declare fields
            allow_declare_fields: true,
            // Remove class fields without initializers (TypeScript-only syntax)
            remove_class_fields_without_initializer: false,
            ..Default::default()
        };

        // Configure JSX transform for React
        // Using Classic runtime (React.createElement) instead of Automatic (jsx from react/jsx-runtime)
        // because:
        // 1. External modules are loaded from CDN as window.React
        // 2. CDN React doesn't export react/jsx-runtime entry point
        // 3. Classic runtime calls React.createElement which works with CDN React
        let jsx = JsxOptions {
            runtime: oxc::transformer::JsxRuntime::Classic,
            // Development mode for better error messages in dev
            development: false,
            ..Default::default()
        };

        let transform_options = TransformOptions {
            typescript,
            jsx,
            ..Default::default()
        };

        Self {
            transform_options,
            printed: String::new(),
            errors: Vec::new(),
        }
    }

    fn execute(
        &mut self,
        source_text: &str,
        source_type: SourceType,
        source_path: &Path,
    ) -> Result<String, Vec<OxcDiagnostic>> {
        self.compile(source_text, source_type, source_path);
        if self.errors.is_empty() {
            Ok(mem::take(&mut self.printed))
        } else {
            Err(mem::take(&mut self.errors))
        }
    }
}

impl CompilerInterface for WebCompiler {
    fn handle_errors(&mut self, errors: Vec<OxcDiagnostic>) {
        self.errors.extend(errors);
    }

    fn after_codegen(&mut self, ret: CodegenReturn) {
        self.printed = ret.code;
    }

    fn transform_options(&self) -> Option<&TransformOptions> {
        Some(&self.transform_options)
    }

    fn codegen_options(&self) -> Option<CodegenOptions> {
        Some(CodegenOptions::default())
    }
}

/// External module configuration for CDN loading
#[derive(Debug, Clone)]
pub struct ExternalModule {
    /// Package name (e.g., "react", "@mui/material")
    pub package: String,
    /// Global variable name (e.g., "React", "MaterialUI")
    pub global: String,
    /// Optional CDN URL for HTML injection
    pub cdn_url: Option<String>,
}

impl ExternalModule {
    /// Create a new external module configuration
    pub fn new(package: &str, global: &str, cdn_url: Option<&str>) -> Self {
        Self {
            package: package.to_string(),
            global: global.to_string(),
            cdn_url: cdn_url.map(|s| s.to_string()),
        }
    }
}

/// Default external modules for React applications
/// NOTE: Only includes packages with CDN URLs. Other packages will be bundled.
/// If you need more packages external, add them with their CDN URLs to your config.
pub fn default_react_externals() -> Vec<ExternalModule> {
    vec![
        // React core - always external, loaded from CDN
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
        // Note: Other packages like @mui/material, @emotion/react, xstate, etc.
        // are NOT external by default because they don't have simple UMD builds.
        // They will be bundled with the application code.
        // To make them external, add them to your config with CDN URLs.
    ]
}

/// Configuration for web bundling
pub struct WebBundleConfig {
    /// Project root directory
    pub project_root: PathBuf,
    /// Source directory (usually src/)
    pub src_dir: PathBuf,
    /// Output directory (usually dist/)
    pub out_dir: PathBuf,
    /// Path to index.html template
    pub index_html: PathBuf,
    /// Entry point (usually src/main.tsx or src/index.tsx)
    pub entry_point: PathBuf,
    /// Public directory for static assets
    pub public_dir: Option<PathBuf>,
    /// Base path for assets (e.g., "/" or "/app/")
    pub base_path: String,
    /// Whether to minify output
    pub minify: bool,
    /// External modules to load from CDN (not bundled)
    /// These map package names to global variables
    /// Default: React ecosystem packages
    pub externals: Vec<ExternalModule>,
    /// Whether to bundle non-external node_modules
    /// When true, packages not in `externals` will be bundled
    /// When false, all node_modules are treated as external
    pub bundle_node_modules: bool,
}

impl Default for WebBundleConfig {
    fn default() -> Self {
        Self {
            project_root: PathBuf::new(),
            src_dir: PathBuf::new(),
            out_dir: PathBuf::new(),
            index_html: PathBuf::new(),
            entry_point: PathBuf::new(),
            public_dir: None,
            base_path: "/".to_string(),
            minify: false,
            externals: default_react_externals(),
            bundle_node_modules: false, // Default to CDN-only for safety
        }
    }
}

/// A map for efficient external module lookups
#[derive(Debug, Default)]
struct ExternalsMap {
    /// Map from package name to global variable name
    package_to_global: HashMap<String, String>,
    /// List of CDN URLs in order
    cdn_urls: Vec<String>,
}

impl ExternalsMap {
    /// Create from a list of external modules
    fn from_externals(externals: &[ExternalModule]) -> Self {
        let mut map = Self::default();
        for ext in externals {
            map.package_to_global
                .insert(ext.package.clone(), ext.global.clone());
            if let Some(url) = &ext.cdn_url {
                map.cdn_urls.push(url.clone());
            }
        }
        map
    }

    /// Check if a package is external
    fn is_external(&self, specifier: &str) -> bool {
        let base_module = get_base_module(specifier);
        self.package_to_global.contains_key(&base_module)
    }

    /// Get the global name for an external package
    fn get_global(&self, specifier: &str) -> Option<String> {
        let base_module = get_base_module(specifier);
        self.package_to_global.get(&base_module).cloned()
    }

    /// Get CDN script tags for HTML injection
    fn get_cdn_scripts(&self) -> String {
        if self.cdn_urls.is_empty() {
            return String::new();
        }
        let scripts: Vec<String> = self
            .cdn_urls
            .iter()
            .map(|url| format!(r#"<script crossorigin src="{}"></script>"#, url))
            .collect();
        format!(
            "<!-- External dependencies from CDN -->\n    {}",
            scripts.join("\n    ")
        )
    }
}

/// Get the base module name from a specifier (handles deep imports)
fn get_base_module(specifier: &str) -> String {
    specifier
        .split('/')
        .take(if specifier.starts_with('@') { 2 } else { 1 })
        .collect::<Vec<_>>()
        .join("/")
}

/// Module in the dependency graph
#[derive(Debug)]
struct Module {
    /// Absolute path to the module
    path: PathBuf,
    /// Original source code
    source: String,
    /// Compiled JavaScript
    compiled: String,
    /// Dependencies (paths to other modules)
    dependencies: Vec<PathBuf>,
    /// Exports provided by this module
    exports: Vec<String>,
    /// Whether this module uses CommonJS syntax
    is_commonjs: bool,
}

/// Track which exports are used across the module graph (for tree shaking)
#[derive(Debug, Default)]
#[cfg_attr(test, derive(Clone))]
pub(crate) struct UsedExportsTracker {
    /// Map from module path to set of used export names
    /// "*" means all exports are used (e.g., via `export * from` or `import *`)
    used_exports: HashMap<PathBuf, HashSet<String>>,
}

impl UsedExportsTracker {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Mark an export as used
    pub(crate) fn mark_used(&mut self, module_path: &Path, export_name: &str) {
        self.used_exports
            .entry(module_path.to_path_buf())
            .or_default()
            .insert(export_name.to_string());
    }

    /// Mark all exports as used (for `import *` or `export *`)
    pub(crate) fn mark_all_used(&mut self, module_path: &Path) {
        self.used_exports
            .entry(module_path.to_path_buf())
            .or_default()
            .insert("*".to_string());
    }

    /// Check if an export is used
    pub(crate) fn is_used(&self, module_path: &Path, export_name: &str) -> bool {
        self.used_exports.get(module_path).map_or(false, |exports| {
            exports.contains("*") || exports.contains(export_name) || exports.contains("default")
        })
    }

    /// Check if any exports are used from a module
    pub(crate) fn has_any_used(&self, module_path: &Path) -> bool {
        self.used_exports
            .get(module_path)
            .map_or(false, |exports| !exports.is_empty())
    }
}

/// Result of bundling
pub struct BundleResult {
    /// Path to the bundled JavaScript file
    pub js_bundle: PathBuf,
    /// Path to the generated index.html
    pub index_html: PathBuf,
    /// Paths to copied assets
    pub assets: Vec<PathBuf>,
    /// Content hash of the bundle (for cache busting)
    pub bundle_hash: String,
}

/// Bundle a web application
pub fn bundle_web_app(config: &WebBundleConfig) -> Result<BundleResult> {
    println!("  Starting web bundler...");

    // Create externals map from configuration
    let externals_map = ExternalsMap::from_externals(&config.externals);

    // Create output directory
    fs::create_dir_all(&config.out_dir)?;

    // Step 1: Build module graph starting from entry point
    println!("    Building module graph...");
    let modules = build_module_graph(&config.entry_point, &config.project_root)?;
    println!("    Found {} modules", modules.len());

    // Step 2: Topologically sort modules (dependencies first)
    let sorted_modules = topological_sort(&modules)?;

    // Step 3: Bundle all modules into a single file
    println!("    Bundling modules...");
    let bundle_content = create_bundle(&sorted_modules, &config.base_path)?;

    // Step 4: Generate content hash for cache busting
    let bundle_hash = generate_hash(&bundle_content);
    let bundle_filename = format!("index-{}.js", &bundle_hash[..8]);

    // Step 5: Write bundle
    let bundle_path = config.out_dir.join(&bundle_filename);
    fs::write(&bundle_path, &bundle_content)?;
    println!("    Bundle written: {}", bundle_filename);

    // Step 6: Copy and process CSS files
    let css_files = copy_css_files(&config.src_dir, &config.out_dir)?;
    let css_tags = css_files
        .iter()
        .map(|p| {
            format!(
                "<link rel=\"stylesheet\" href=\"{}{}\" />",
                config.base_path,
                p.file_name().unwrap().to_string_lossy()
            )
        })
        .collect::<Vec<_>>()
        .join("\n    ");

    // Step 7: Copy public assets
    let mut assets = Vec::new();
    if let Some(public_dir) = &config.public_dir {
        if public_dir.exists() {
            assets = copy_public_assets(public_dir, &config.out_dir)?;
        }
    }

    // Step 8: Generate index.html with configured CDN scripts
    println!("    Generating index.html...");
    let cdn_scripts = externals_map.get_cdn_scripts();
    let index_html_path = generate_index_html(
        &config.index_html,
        &config.out_dir,
        &bundle_filename,
        &css_tags,
        &config.base_path,
        &cdn_scripts,
    )?;

    println!("  Web bundler complete!");

    Ok(BundleResult {
        js_bundle: bundle_path,
        index_html: index_html_path,
        assets,
        bundle_hash,
    })
}

/// Build module graph starting from entry point
fn build_module_graph(entry: &Path, project_root: &Path) -> Result<HashMap<PathBuf, Module>> {
    let mut modules = HashMap::new();
    let mut queue = vec![entry.to_path_buf()];
    let mut visited = HashSet::new();

    // Configure resolver for TypeScript/React projects
    let resolve_options = ResolveOptions {
        extensions: vec![
            ".tsx".into(),
            ".ts".into(),
            ".jsx".into(),
            ".js".into(),
            ".mjs".into(),
            ".cjs".into(),
            ".json".into(),
        ],
        main_fields: vec!["module".into(), "main".into()],
        condition_names: vec!["import".into(), "module".into(), "default".into()],
        alias_fields: vec![vec!["browser".into()]],
        ..Default::default()
    };

    let resolver = Resolver::new(resolve_options);

    while let Some(module_path) = queue.pop() {
        if visited.contains(&module_path) {
            continue;
        }
        visited.insert(module_path.clone());

        // Skip node_modules - these are external dependencies
        if module_path.to_string_lossy().contains("node_modules") {
            continue;
        }

        // Read and parse the module
        let source = fs::read_to_string(&module_path)
            .with_context(|| format!("Failed to read module: {}", module_path.display()))?;

        // Determine source type
        let source_type = SourceType::from_path(&module_path)
            .map_err(|e| anyhow::anyhow!("Invalid source type: {:?}", e))?;

        // Compile the module with transforms enabled (TypeScript → JavaScript, JSX → createElement)
        let mut compiler = WebCompiler::new();
        let compiled = match compiler.execute(&source, source_type, &module_path) {
            Ok(output) => output,
            Err(errors) => {
                let error_msgs: Vec<_> = errors.iter().map(|e| format!("{:?}", e)).collect();
                anyhow::bail!(
                    "Compilation failed for {}: {}",
                    module_path.display(),
                    error_msgs.join(", ")
                );
            }
        };

        // Analyze the module to find dependencies, exports, and detect module system
        let analysis = analyze_module(&source, &module_path, &resolver, project_root)?;

        // Add dependencies to queue
        for dep in &analysis.imports {
            if !visited.contains(dep) {
                queue.push(dep.clone());
            }
        }

        modules.insert(
            module_path.clone(),
            Module {
                path: module_path,
                source,
                compiled,
                dependencies: analysis.imports,
                exports: analysis.exports,
                is_commonjs: analysis.is_commonjs,
            },
        );
    }

    Ok(modules)
}

/// Result of analyzing a module's imports, exports, and module system
struct ModuleAnalysis {
    /// Import dependencies
    imports: Vec<PathBuf>,
    /// Exported names
    exports: Vec<String>,
    /// Whether this module uses CommonJS (module.exports or exports.X)
    is_commonjs: bool,
}

/// Extract imports, exports, and detect module system from source code
fn analyze_module(
    source: &str,
    module_path: &Path,
    resolver: &Resolver,
    _project_root: &Path,
) -> Result<ModuleAnalysis> {
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut is_commonjs = false;

    // Regex patterns for various import syntaxes
    let import_regex = Regex::new(r#"(?:import|export)\s+(?:[\s\S]*?from\s+)?['"]([^'"]+)['"]"#)?;
    let require_regex = Regex::new(r#"require\s*\(\s*['"]([^'"]+)['"]\s*\)"#)?;

    // CommonJS detection patterns
    let module_exports_regex = Regex::new(r#"module\.exports\s*[=.]"#)?;
    let exports_dot_regex = Regex::new(r#"exports\.(\w+)\s*="#)?;

    // ESM export patterns for extracting export names
    let export_default_regex = Regex::new(r#"export\s+default\s+"#)?;
    let export_named_regex =
        Regex::new(r#"export\s+(?:const|let|var|function|class|async\s+function)\s+(\w+)"#)?;
    let export_list_regex = Regex::new(r#"export\s*\{([^}]+)\}"#)?;

    let parent_dir = module_path.parent().unwrap_or(Path::new("."));

    // Extract imports
    for cap in import_regex.captures_iter(source) {
        if let Some(specifier) = cap.get(1) {
            let spec = specifier.as_str();

            // Skip external dependencies (from node_modules)
            if !spec.starts_with('.') && !spec.starts_with('/') {
                continue;
            }

            // Resolve the import
            if let Ok(resolved) = resolver.resolve(parent_dir, spec) {
                let resolved_path = resolved.full_path();
                // Only include local files (not node_modules)
                if !resolved_path.to_string_lossy().contains("node_modules") {
                    imports.push(resolved_path);
                }
            }
        }
    }

    for cap in require_regex.captures_iter(source) {
        if let Some(specifier) = cap.get(1) {
            let spec = specifier.as_str();
            if !spec.starts_with('.') && !spec.starts_with('/') {
                continue;
            }
            if let Ok(resolved) = resolver.resolve(parent_dir, spec) {
                let resolved_path = resolved.full_path();
                if !resolved_path.to_string_lossy().contains("node_modules") {
                    imports.push(resolved_path);
                }
            }
        }
    }

    // Detect CommonJS
    if module_exports_regex.is_match(source) {
        is_commonjs = true;
    }
    for cap in exports_dot_regex.captures_iter(source) {
        is_commonjs = true;
        if let Some(name) = cap.get(1) {
            exports.push(name.as_str().to_string());
        }
    }

    // Extract ESM exports
    if export_default_regex.is_match(source) {
        exports.push("default".to_string());
    }

    for cap in export_named_regex.captures_iter(source) {
        if let Some(name) = cap.get(1) {
            exports.push(name.as_str().to_string());
        }
    }

    for cap in export_list_regex.captures_iter(source) {
        if let Some(list) = cap.get(1) {
            // Parse export list: { a, b as c, d }
            for item in list.as_str().split(',') {
                let item = item.trim();
                if item.is_empty() || item.starts_with("type ") {
                    continue;
                }
                // Handle "a as b" - the exported name is "b"
                let exported = if let Some(as_pos) = item.find(" as ") {
                    item[as_pos + 4..].trim()
                } else {
                    item
                };
                exports.push(exported.to_string());
            }
        }
    }

    Ok(ModuleAnalysis {
        imports,
        exports,
        is_commonjs,
    })
}

/// Topologically sort modules (dependencies before dependents)
fn topological_sort(modules: &HashMap<PathBuf, Module>) -> Result<Vec<&Module>> {
    let mut sorted = Vec::new();
    let mut visited = HashSet::new();
    let mut temp_visited = HashSet::new();

    fn visit<'a>(
        module_path: &PathBuf,
        modules: &'a HashMap<PathBuf, Module>,
        visited: &mut HashSet<PathBuf>,
        temp_visited: &mut HashSet<PathBuf>,
        sorted: &mut Vec<&'a Module>,
    ) -> Result<()> {
        if visited.contains(module_path) {
            return Ok(());
        }
        if temp_visited.contains(module_path) {
            // Circular dependency - this is okay for ES modules
            return Ok(());
        }

        temp_visited.insert(module_path.clone());

        if let Some(module) = modules.get(module_path) {
            for dep in &module.dependencies {
                if modules.contains_key(dep) {
                    visit(dep, modules, visited, temp_visited, sorted)?;
                }
            }

            visited.insert(module_path.clone());
            sorted.push(module);
        }

        temp_visited.remove(module_path);
        Ok(())
    }

    for path in modules.keys() {
        visit(path, modules, &mut visited, &mut temp_visited, &mut sorted)?;
    }

    Ok(sorted)
}

/// Create the bundle by concatenating modules
fn create_bundle(modules: &[&Module], _base_path: &str) -> Result<String> {
    let mut bundle = String::new();

    // Build a map of all module paths for resolution
    let module_paths: HashSet<String> = modules
        .iter()
        .map(|m| m.path.to_string_lossy().to_string())
        .collect();

    // Add module system wrapper with CommonJS/ESM interop support
    // Features:
    // - __esModule marker for proper default export handling
    // - Circular dependency detection (returns partial exports)
    // - Proper CommonJS/ESM interop via __toESM and __toCommonJS helpers
    bundle.push_str(
        r#"(function() {
  'use strict';
  var __modules = {};
  var __exports = {};
  var __loading = {}; // Track modules currently being loaded (for circular deps)

  // Helper to mark a module as ESM
  function __markESM(exports) {
    if (typeof Symbol !== 'undefined' && Symbol.toStringTag) {
      Object.defineProperty(exports, Symbol.toStringTag, { value: 'Module' });
    }
    Object.defineProperty(exports, '__esModule', { value: true });
    return exports;
  }

  // Convert ESM to CommonJS-compatible (handles default export)
  function __toCommonJS(mod) {
    return mod && mod.__esModule ? mod : { default: mod, ...mod };
  }

  // Convert CommonJS to ESM-compatible (handles default export)
  function __toESM(mod) {
    if (mod && mod.__esModule) return mod;
    var result = {};
    if (mod != null) {
      for (var k in mod) {
        if (Object.prototype.hasOwnProperty.call(mod, k)) {
          result[k] = mod[k];
        }
      }
    }
    result.default = mod;
    return result;
  }

  function __require(id) {
    // Return cached exports if already loaded
    if (__exports[id]) return __exports[id];

    // Handle circular dependencies - return partial exports
    if (__loading[id]) {
      console.debug('[Bundle] Circular dependency detected: ' + id);
      return __exports[id] || {};
    }

    if (!__modules[id]) {
      console.warn('[Bundle] Module not found: ' + id);
      return {};
    }

    // Mark as loading (for circular dep detection)
    __loading[id] = true;

    // Create module object with empty exports
    var module = { exports: {} };
    __exports[id] = module.exports;

    // Execute module
    try {
      __modules[id](module, module.exports, __require, __toESM, __toCommonJS);
      __exports[id] = module.exports;
    } finally {
      delete __loading[id];
    }

    return __exports[id];
  }

"#,
    );

    // Add each module
    for module in modules {
        let module_id = module.path.to_string_lossy();

        // Wrap module code in a function
        // Pass __toESM and __toCommonJS helpers for interop
        bundle.push_str(&format!(
            "  __modules['{}'] = function(module, exports, require, __toESM, __toCommonJS) {{\n",
            module_id
        ));

        // Add __esModule marker for ESM modules (helps with interop)
        if !module.is_commonjs {
            bundle.push_str("    __markESM(exports);\n");
        }

        // Add the compiled code (with transforms for imports/exports)
        // Pass the module path so relative imports can be resolved to absolute paths
        let code = transform_module_code(&module.compiled, Some(&module.path), &module_paths);
        for line in code.lines() {
            bundle.push_str("    ");
            bundle.push_str(line);
            bundle.push('\n');
        }

        bundle.push_str("  };\n\n");
    }

    // Add entry point execution
    if let Some(entry) = modules.last() {
        bundle.push_str(&format!(
            "  __require('{}');\n",
            entry.path.to_string_lossy()
        ));
    }

    bundle.push_str("})();\n");

    Ok(bundle)
}

/// Transform module code to work with our simple module system using AST
/// This converts ES modules to CommonJS-compatible format
///
/// Uses OXC's parser to properly understand the AST structure instead of regex,
/// which handles edge cases like:
/// - Import/export statements inside string literals
/// - Complex export patterns
/// - All ES module syntax variations
///
/// When `current_module_path` is provided, relative imports are resolved to
/// absolute paths that match the module registry.
fn transform_module_code(
    code: &str,
    current_module_path: Option<&Path>,
    known_modules: &HashSet<String>,
) -> String {
    // Parse the code into an AST
    // Use TypeScript source type to handle any remaining TS syntax
    let allocator = Allocator::default();
    let source_type = SourceType::default()
        .with_module(true)
        .with_typescript(true); // Parse as TypeScript to handle edge cases
    let ret = Parser::new(&allocator, code, source_type)
        .with_options(ParseOptions::default())
        .parse();

    // If parsing failed, fall back to the original code
    // (OXC compiler output should always parse successfully)
    if !ret.errors.is_empty() {
        return code.to_string();
    }

    // Get the directory of the current module for resolving relative imports
    let current_dir = current_module_path.and_then(|p| p.parent());

    // Collect all transformations: (start, end, replacement)
    let mut transforms: Vec<(usize, usize, String)> = Vec::new();

    for stmt in &ret.program.body {
        match stmt {
            // =====================================================
            // Import Declarations
            // =====================================================
            Statement::ImportDeclaration(import) => {
                let replacement =
                    transform_import_declaration(import, current_dir, known_modules);
                transforms.push((
                    import.span.start as usize,
                    import.span.end as usize,
                    replacement,
                ));
            }

            // =====================================================
            // Export Declarations
            // =====================================================
            Statement::ExportAllDeclaration(export_all) => {
                // export * from 'module' or export * as name from 'module'
                let specifier = export_all.source.value.as_str();
                let resolved = resolve_module_path(specifier, current_dir, known_modules);
                let replacement = if let Some(exported) = &export_all.exported {
                    // export * as name from 'module'
                    let name = get_module_export_name(exported);
                    if is_external_module(specifier) {
                        format!(
                            "module.exports.{} = window.{} || {{}};",
                            name,
                            get_global_name(specifier)
                        )
                    } else {
                        format!("module.exports.{} = require('{}');", name, resolved)
                    }
                } else {
                    // export * from 'module'
                    if is_external_module(specifier) {
                        format!(
                            "Object.assign(module.exports, window.{} || {{}});",
                            get_global_name(specifier)
                        )
                    } else {
                        format!("Object.assign(module.exports, require('{}'));", resolved)
                    }
                };
                transforms.push((
                    export_all.span.start as usize,
                    export_all.span.end as usize,
                    replacement,
                ));
            }

            Statement::ExportDefaultDeclaration(export_default) => {
                let replacement = transform_export_default(export_default, code);
                transforms.push((
                    export_default.span.start as usize,
                    export_default.span.end as usize,
                    replacement,
                ));
            }

            Statement::ExportNamedDeclaration(export_named) => {
                let replacement =
                    transform_export_named(export_named, code, current_dir, known_modules);
                transforms.push((
                    export_named.span.start as usize,
                    export_named.span.end as usize,
                    replacement,
                ));
            }

            // Skip TypeScript-only exports (already stripped by OXC compiler)
            Statement::TSExportAssignment(_) | Statement::TSNamespaceExportDeclaration(_) => {
                // These should have been removed by OXC compiler, but just in case
            }

            _ => {}
        }
    }

    // Apply transformations from back to front to preserve earlier spans
    transforms.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = code.to_string();
    for (start, end, replacement) in transforms {
        if start <= result.len() && end <= result.len() {
            result.replace_range(start..end, &replacement);
        }
    }

    // Transform Vite-specific import.meta.env to use window.ENV
    // NovaSkyn loads env.js which sets window.ENV
    // Common patterns:
    //   import.meta.env.VITE_API_URL → (window.ENV && window.ENV.VITE_API_URL)
    //   import.meta.env.MODE → (window.ENV && window.ENV.MODE || 'production')
    //   import.meta.env.DEV → false
    //   import.meta.env.PROD → true
    //   import.meta.env.SSR → false
    //   import.meta.env.BASE_URL → '/'
    let result = result
        .replace("import.meta.env.DEV", "false")
        .replace("import.meta.env.PROD", "true")
        .replace("import.meta.env.SSR", "false")
        .replace("import.meta.env.BASE_URL", "'/'")
        .replace("import.meta.env.MODE", "'production'");

    // For other import.meta.env.VITE_* variables, replace with window.ENV access
    let import_meta_env_re = Regex::new(r"import\.meta\.env\.(\w+)").unwrap();
    let result = import_meta_env_re
        .replace_all(&result, |caps: &regex::Captures| {
            let var_name = &caps[1];
            format!("(window.ENV && window.ENV.{})", var_name)
        })
        .to_string();

    // Handle bare import.meta.env (accessing the whole object)
    let result = result.replace("import.meta.env", "(window.ENV || {})");

    // Handle import.meta.url (used for dynamic imports and asset paths)
    let result = result.replace("import.meta.url", "window.location.href");

    result
}

/// Resolve a module specifier to an absolute path if it's a local module
fn resolve_module_path(
    specifier: &str,
    current_dir: Option<&Path>,
    known_modules: &HashSet<String>,
) -> String {
    // External modules don't need resolution
    if is_external_module(specifier) {
        return specifier.to_string();
    }

    // If no current directory, can't resolve
    let Some(dir) = current_dir else {
        return specifier.to_string();
    };

    // Try to resolve the relative path
    let base_path = dir.join(specifier);

    // Check if the specifier already has a recognized file extension
    // Look at the filename part only (not the whole path which may contain ./)
    // Only consider actual JS/TS extensions, not naming conventions like .machine.ts
    let recognized_extensions = [
        ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".json", ".css", ".scss", ".less",
    ];
    let has_extension = Path::new(specifier)
        .file_name()
        .and_then(|f| f.to_str())
        .map(|filename| {
            // Check if filename ends with a recognized extension
            recognized_extensions
                .iter()
                .any(|ext| filename.ends_with(ext))
        })
        .unwrap_or(false);

    // Try common extensions
    let extensions = [".tsx", ".ts", ".jsx", ".js", ".mjs", ".cjs", ".json", ""];
    for ext in extensions {
        let candidate = if ext.is_empty() {
            base_path.to_string_lossy().to_string()
        } else if has_extension {
            // Already has extension, don't add more
            base_path.to_string_lossy().to_string()
        } else {
            format!("{}{}", base_path.to_string_lossy(), ext)
        };

        // Normalize the path (resolve . and ..)
        if let Ok(normalized) = std::fs::canonicalize(&candidate) {
            let normalized_str = normalized.to_string_lossy().to_string();
            if known_modules.contains(&normalized_str) {
                return normalized_str;
            }
        }

        // Also try without canonicalize for paths that might not exist on disk
        // (e.g., during testing)
        if known_modules.contains(&candidate) {
            return candidate;
        }
    }

    // Also try index files
    for ext in [".tsx", ".ts", ".jsx", ".js"] {
        let index_path = format!("{}/index{}", base_path.to_string_lossy(), ext);
        if let Ok(normalized) = std::fs::canonicalize(&index_path) {
            let normalized_str = normalized.to_string_lossy().to_string();
            if known_modules.contains(&normalized_str) {
                return normalized_str;
            }
        }
        if known_modules.contains(&index_path) {
            return index_path;
        }
    }

    // Return original if no match found
    specifier.to_string()
}

/// Transform an import declaration to CommonJS
fn transform_import_declaration(
    import: &ImportDeclaration,
    current_dir: Option<&Path>,
    known_modules: &HashSet<String>,
) -> String {
    let specifier = import.source.value.as_str();
    let resolved = resolve_module_path(specifier, current_dir, known_modules);

    // Handle type-only imports (should be stripped by OXC, but just in case)
    if import.import_kind.is_type() {
        return String::new();
    }

    // No specifiers = side-effect import: import 'module'
    if import.specifiers.as_ref().map_or(true, |s| s.is_empty()) {
        return if is_external_module(specifier) {
            "/* external side-effect import */".to_string()
        } else {
            format!("require('{}');", specifier)
        };
    }

    let specifiers = import.specifiers.as_ref().unwrap();
    let mut parts = Vec::new();

    // Separate specifiers by type
    let mut default_import: Option<String> = None;
    let mut namespace_import: Option<String> = None;
    let mut named_imports: Vec<(String, String)> = Vec::new(); // (local, imported)

    for spec in specifiers {
        match spec {
            ImportDeclarationSpecifier::ImportDefaultSpecifier(default) => {
                default_import = Some(default.local.name.to_string());
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(namespace) => {
                namespace_import = Some(namespace.local.name.to_string());
            }
            ImportDeclarationSpecifier::ImportSpecifier(named) => {
                let local = named.local.name.to_string();
                let imported = get_module_export_name(&named.imported);
                named_imports.push((local, imported));
            }
        }
    }

    if is_external_module(specifier) {
        let global = get_global_name(specifier);

        // Handle namespace import: import * as X from 'module'
        if let Some(name) = namespace_import {
            return format!("var {} = window.{} || {{}};", name, global);
        }

        // Handle default import
        if let Some(name) = &default_import {
            parts.push(format!("var __{g} = window.{g} || {{}};", g = global));
            parts.push(format!(
                "var {n} = __{g}.default || __{g};",
                n = name,
                g = global
            ));
        }

        // Handle named imports
        if !named_imports.is_empty() {
            if default_import.is_none() {
                parts.push(format!("var __{g} = window.{g} || {{}};", g = global));
            }
            let destructure: Vec<String> = named_imports
                .iter()
                .map(|(local, imported)| {
                    if local == imported {
                        local.clone()
                    } else {
                        format!("{}: {}", imported, local)
                    }
                })
                .collect();
            parts.push(format!(
                "var {{ {} }} = __{};",
                destructure.join(", "),
                global
            ));
        }
    } else {
        // Local module - use resolved path to match module registry
        let temp_var = format!("__mod_{}", resolved.replace(['/', '.', '-', '@'], "_"));

        // Handle namespace import: import * as X from 'module'
        if let Some(name) = namespace_import {
            return format!("var {} = require('{}');", name, resolved);
        }

        parts.push(format!("var {} = require('{}');", temp_var, resolved));

        // Handle default import
        if let Some(name) = default_import {
            parts.push(format!(
                "var {n} = {t}.default || {t};",
                n = name,
                t = temp_var
            ));
        }

        // Handle named imports
        if !named_imports.is_empty() {
            let destructure: Vec<String> = named_imports
                .iter()
                .map(|(local, imported)| {
                    if local == imported {
                        local.clone()
                    } else {
                        format!("{}: {}", imported, local)
                    }
                })
                .collect();
            parts.push(format!(
                "var {{ {} }} = {};",
                destructure.join(", "),
                temp_var
            ));
        }
    }

    parts.join(" ")
}

/// Transform an export default declaration to CommonJS
fn transform_export_default(
    export: &oxc::ast::ast::ExportDefaultDeclaration,
    source: &str,
) -> String {
    // Get the span of the declaration (after "export default")
    let decl_start = export.declaration.span().start as usize;
    let decl_end = export.declaration.span().end as usize;
    let decl_code = &source[decl_start..decl_end];

    match &export.declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
            // export default function name() {} -> function name() {}; module.exports.default = name;
            if let Some(id) = func.id.as_ref() {
                let name = id.name.to_string();
                format!("{} module.exports.default = {};", decl_code, name)
            } else {
                // Anonymous function - wrap it
                format!("module.exports.default = {};", decl_code)
            }
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            // export default class Name {} -> class Name {}; module.exports.default = Name;
            if let Some(id) = class.id.as_ref() {
                let name = id.name.to_string();
                format!("{} module.exports.default = {};", decl_code, name)
            } else {
                // Anonymous class - wrap it
                format!("module.exports.default = {};", decl_code)
            }
        }
        _ => {
            // export default <expression>
            format!("module.exports.default = {};", decl_code)
        }
    }
}

/// Transform an export named declaration to CommonJS
fn transform_export_named(
    export: &ExportNamedDeclaration,
    source: &str,
    current_dir: Option<&Path>,
    known_modules: &HashSet<String>,
) -> String {
    // Handle type-only exports (should be stripped by OXC, but just in case)
    if export.export_kind.is_type() {
        return String::new();
    }

    // Re-export: export { a, b } from 'module'
    if let Some(src) = &export.source {
        let specifier = src.value.as_str();
        if export.specifiers.is_empty() {
            return String::new(); // Empty re-export
        }

        // Resolve the module path for local modules
        let resolved = resolve_module_path(specifier, current_dir, known_modules);
        let source_expr = if is_external_module(specifier) {
            format!("window.{} || {{}}", get_global_name(specifier))
        } else {
            format!("require('{}')", resolved)
        };

        let assignments: Vec<String> = export
            .specifiers
            .iter()
            .map(|spec| {
                let local = get_module_export_name(&spec.local);
                let exported = get_module_export_name(&spec.exported);
                format!("module.exports.{} = __reexport.{}", exported, local)
            })
            .collect();

        return format!(
            "(function() {{ var __reexport = {}; {} }})();",
            source_expr,
            assignments.join("; ")
        );
    }

    // Export declaration: export const/let/var/function/class
    if let Some(decl) = &export.declaration {
        // Extract the declaration code using its span
        let decl_start = decl.span().start as usize;
        let decl_end = decl.span().end as usize;
        let decl_code = &source[decl_start..decl_end];

        // Extract declared names and create exports
        let names = get_declaration_names(decl);
        if names.is_empty() {
            return decl_code.to_string();
        }

        let exports: Vec<String> = names
            .iter()
            .map(|n| format!("module.exports.{n} = {n};"))
            .collect();

        return format!("{} {}", decl_code, exports.join(" "));
    }

    // Named export: export { a, b } or export { a as b }
    if !export.specifiers.is_empty() {
        let exports: Vec<String> = export
            .specifiers
            .iter()
            .map(|spec| {
                let local = get_module_export_name(&spec.local);
                let exported = get_module_export_name(&spec.exported);
                format!("module.exports.{} = {};", exported, local)
            })
            .collect();
        return exports.join(" ");
    }

    String::new()
}

/// Get the string name from a ModuleExportName
fn get_module_export_name(name: &ModuleExportName) -> String {
    match name {
        ModuleExportName::IdentifierName(id) => id.name.to_string(),
        ModuleExportName::IdentifierReference(id) => id.name.to_string(),
        ModuleExportName::StringLiteral(s) => s.value.to_string(),
    }
}

/// Get declared variable/function/class names from a declaration
fn get_declaration_names(decl: &oxc::ast::ast::Declaration) -> Vec<String> {
    use oxc::ast::ast::Declaration;

    match decl {
        Declaration::VariableDeclaration(var_decl) => var_decl
            .declarations
            .iter()
            .filter_map(|d| {
                // Handle simple identifier bindings
                if let oxc::ast::ast::BindingPatternKind::BindingIdentifier(id) = &d.id.kind {
                    Some(id.name.to_string())
                } else {
                    None
                }
            })
            .collect(),
        Declaration::FunctionDeclaration(func) => func
            .id
            .as_ref()
            .map(|id| vec![id.name.to_string()])
            .unwrap_or_default(),
        Declaration::ClassDeclaration(class) => class
            .id
            .as_ref()
            .map(|id| vec![id.name.to_string()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Check if a module specifier refers to an external (node_modules) package
fn is_external_module(specifier: &str) -> bool {
    // External if not starting with . or / or @/ (path alias)
    !specifier.starts_with('.') && !specifier.starts_with('/') && !specifier.starts_with("@/")
}

/// Get the global variable name for an external module
fn get_global_name(specifier: &str) -> String {
    // Handle deep imports like "react-dom/client"
    let base_module = specifier
        .split('/')
        .take(if specifier.starts_with('@') { 2 } else { 1 })
        .collect::<Vec<_>>()
        .join("/");

    match base_module.as_str() {
        "react" => "React".to_string(),
        "react-dom" => "ReactDOM".to_string(),
        "react-router-dom" => "ReactRouterDOM".to_string(),
        "@mui/material" | "@mui/styles" | "@mui/system" => "MaterialUI".to_string(),
        "@mui/icons-material" => "MaterialIcons".to_string(),
        "@apollo/client" => "Apollo".to_string(),
        "@tanstack/react-query" => "ReactQuery".to_string(),
        "xstate" => "XState".to_string(),
        "@xstate/react" => "XStateReact".to_string(),
        "zustand" => "Zustand".to_string(),
        "date-fns" => "dateFns".to_string(),
        "zod" => "Zod".to_string(),
        _ => {
            // Convert @scope/name to ScopeName, or name to Name
            let clean = base_module
                .replace("@", "")
                .replace("/", "_")
                .replace("-", "_");
            // Capitalize first letter
            let mut chars = clean.chars();
            match chars.next() {
                None => clean,
                Some(c) => c.to_uppercase().chain(chars).collect(),
            }
        }
    }
}

/// Generate content hash for cache busting
fn generate_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Copy CSS files to output directory
fn copy_css_files(src_dir: &Path, out_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut css_files = Vec::new();

    for entry in walkdir::WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "css"))
    {
        let css_path = entry.path();
        let relative = css_path.strip_prefix(src_dir)?;
        let output_path = out_dir.join(relative);

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Read, potentially process, and write CSS
        let css_content = fs::read_to_string(css_path)?;
        let hash = generate_hash(&css_content);
        let stem = output_path.file_stem().unwrap().to_string_lossy();
        let hashed_name = format!("{}-{}.css", stem, &hash[..8]);
        let hashed_path = output_path.parent().unwrap().join(&hashed_name);

        fs::write(&hashed_path, css_content)?;
        css_files.push(hashed_path);
    }

    Ok(css_files)
}

/// Copy public assets to output directory
fn copy_public_assets(public_dir: &Path, out_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut assets = Vec::new();

    for entry in walkdir::WalkDir::new(public_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
    {
        let src_path = entry.path();
        let relative = src_path.strip_prefix(public_dir)?;
        let dest_path = out_dir.join(relative);

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(src_path, &dest_path)?;
        assets.push(dest_path);
    }

    Ok(assets)
}

/// Generate index.html with proper script and style tags
fn generate_index_html(
    template_path: &Path,
    out_dir: &Path,
    bundle_filename: &str,
    css_tags: &str,
    base_path: &str,
    cdn_scripts: &str,
) -> Result<PathBuf> {
    let template = if template_path.exists() {
        fs::read_to_string(template_path)?
    } else {
        // Default template if none provided
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>App</title>
    <!-- CSS_PLACEHOLDER -->
</head>
<body>
    <div id="root"></div>
    <!-- SCRIPT_PLACEHOLDER -->
</body>
</html>"#
            .to_string()
    };

    // Build script tag with CDN scripts loaded before our bundle
    let script_tag = if cdn_scripts.is_empty() {
        format!("<script src=\"{}{}\"></script>", base_path, bundle_filename)
    } else {
        format!(
            "{}\n    <script src=\"{}{}\"></script>",
            cdn_scripts, base_path, bundle_filename
        )
    };

    let html = template
        .replace("<!-- CSS_PLACEHOLDER -->", css_tags)
        .replace("<!-- SCRIPT_PLACEHOLDER -->", &script_tag)
        // Also handle Vite-style placeholders
        .replace(
            r#"<script type="module" src="/src/main.tsx"></script>"#,
            &script_tag,
        )
        .replace(
            r#"<script type="module" src="./src/main.tsx"></script>"#,
            &script_tag,
        );

    let output_path = out_dir.join("index.html");
    fs::write(&output_path, html)?;

    Ok(output_path)
}
