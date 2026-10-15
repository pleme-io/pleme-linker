//! TypeScript/JavaScript project building (for non-Vite use cases)
//!
//! This fills the gap that Vite fills for frontend projects.
//! For backend projects (like MCP servers, GitHub Actions), we need:
//! 1. node_modules built from deps.nix
//! 2. TypeScript compilation (native Rolldown or fallback to tsc)
//!    OR direct copy for plain JavaScript projects
//! 3. Workspace dependency handling
//! 4. Wrapper script creation
//!
//! Auto-detects project type:
//! - If tsconfig.json exists: TypeScript mode (OXC compilation)
//! - If no tsconfig.json: JavaScript mode (copy source files directly)
//!
//! With --native-compile (default), uses pure Rust Rolldown bundler.
//! With --use-tsc, falls back to shelling out to tsc.

use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::build::run_build;
use crate::cli::{BuildArgs, BuildProjectArgs};
use crate::rolldown_bundler;
use crate::swc_compiler::generate_declarations;
use crate::utils::copy_dir_recursive;
use crate::web_bundler::{self, WebBundleConfig};

/// Track both built path and source path for workspace deps
/// (source path needed for tsconfig project references)
struct WorkspaceDepInfo {
    name: String,
    built_path: PathBuf,
    source_path: Option<PathBuf>,
}

/// Copy JavaScript source files (preserving directory structure) for JS-mode builds
fn copy_js_source_files(src_dir: &Path, out_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut outputs = Vec::new();
    fs::create_dir_all(out_dir)?;

    for entry in walkdir::WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file()
                && path.extension().map_or(false, |ext| {
                    ext == "js" || ext == "mjs" || ext == "cjs" || ext == "json"
                })
        })
    {
        let source_path = entry.path();
        let relative_path = source_path.strip_prefix(src_dir).unwrap_or(source_path);
        let output_path = out_dir.join(relative_path);

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        println!("    Copying: {}", relative_path.display());
        fs::copy(source_path, &output_path)?;
        outputs.push(output_path);
    }

    Ok(outputs)
}

/// Run the build-project command
pub fn run_build_project(args: BuildProjectArgs) -> Result<()> {
    // Auto-detect project type based on tsconfig.json presence
    let has_tsconfig = args.project.join("tsconfig.json").exists();
    let project_kind = if has_tsconfig { "TypeScript" } else { "JavaScript" };
    println!("pleme-linker build-project: Building {} project", project_kind);
    println!("  Project:  {}", args.project.display());
    println!("  Output:   {}", args.output.display());
    println!("  Manifest: {}", args.manifest.display());
    println!();

    // Create output directories
    let lib_dir = args.output.join("lib");
    let bin_dir = args.output.join("bin");
    fs::create_dir_all(&lib_dir)?;
    fs::create_dir_all(&bin_dir)?;

    // =========================================================================
    // STAGE 1: Build node_modules using existing build logic
    // =========================================================================
    println!("Stage 1: Building node_modules...");

    let node_modules_dir = lib_dir.join("node_modules_build");
    run_build(BuildArgs {
        manifest: args.manifest.clone(),
        output: node_modules_dir.clone(),
        node_bin: args.node_bin.clone(),
    })?;

    // =========================================================================
    // STAGE 1.5: Build workspace packages from source (if any)
    // =========================================================================
    let mut built_workspace_deps: Vec<WorkspaceDepInfo> = args
        .workspace_dep
        .iter()
        .map(|(name, path)| WorkspaceDepInfo {
            name: name.clone(),
            built_path: path.clone(),
            source_path: None,
        })
        .collect();

    if !args.workspace_src.is_empty() {
        println!();
        println!("Stage 1.5: Building workspace packages from source...");

        let workspace_build_root = lib_dir.join("workspace_builds");
        fs::create_dir_all(&workspace_build_root)?;

        for ws in &args.workspace_src {
            println!("  Building workspace package: {}", ws.name);

            let safe_name = ws.name.replace('@', "").replace('/', "-");
            let ws_output = workspace_build_root.join(&safe_name);
            fs::create_dir_all(&ws_output)?;

            let deps_for_recursive: Vec<(String, PathBuf)> = built_workspace_deps
                .iter()
                .map(|d| (d.name.clone(), d.built_path.clone()))
                .collect();

            build_workspace_package(
                &ws.name,
                &ws.manifest,
                &ws.src,
                &ws_output,
                &args.node_bin,
                args.parent_tsconfig.as_deref(),
                &deps_for_recursive,
            )?;

            built_workspace_deps.push(WorkspaceDepInfo {
                name: ws.name.clone(),
                built_path: ws_output.join("lib"),
                source_path: Some(ws.src.clone()),
            });
        }
    }

    // =========================================================================
    // STAGE 2: Set up build directory with source and dependencies
    // =========================================================================
    println!();
    println!("Stage 2: Setting up build environment...");

    let build_dir = lib_dir.join("build_workspace");
    fs::create_dir_all(&build_dir)?;

    // Copy project source to build directory
    copy_dir_recursive(&args.project, &build_dir)?;

    // Symlink node_modules into build directory
    let build_node_modules = build_dir.join("node_modules");
    if build_node_modules.exists() {
        fs::remove_dir_all(&build_node_modules)?;
    }
    std::os::unix::fs::symlink(node_modules_dir.join("node_modules"), &build_node_modules)?;

    // Set up parent tsconfig if specified
    if let Some(parent_tsconfig) = &args.parent_tsconfig {
        let parent_dest = build_dir.parent().unwrap().join("tsconfig.json");
        fs::copy(parent_tsconfig, &parent_dest)?;
    }

    // Set up workspace dependencies
    for dep_info in &built_workspace_deps {
        setup_workspace_dep(&build_dir, &build_node_modules, dep_info)?;
    }

    // =========================================================================
    // STAGE 3: Detect project type and compile accordingly
    // =========================================================================
    println!();

    // Detect if this is a web application (has index.html and src/main.tsx)
    let index_html = build_dir.join("index.html");
    let src_dir = build_dir.join("src");
    let main_tsx = src_dir.join("main.tsx");
    let index_tsx = src_dir.join("index.tsx");
    let is_web_app = index_html.exists() && (main_tsx.exists() || index_tsx.exists());

    let dist_dir = build_dir.join("dist");
    fs::create_dir_all(&dist_dir)?;

    if is_web_app {
        // =====================================================================
        // WEB APP BUILD: Use Vite or pure Rust OXC bundler
        // =====================================================================
        if args.use_vite {
            // Use Vite bundler (shell out to npx vite build)
            println!("Stage 3: Building web application with Vite...");

            let status = Command::new(&args.node_bin)
                .args(["--", &build_node_modules.join(".bin/vite").to_string_lossy(), "build"])
                .current_dir(&build_dir)
                .env("NODE_ENV", "production")
                .status()
                .with_context(|| "Failed to run Vite build")?;

            if !status.success() {
                anyhow::bail!("Vite build failed with exit code: {:?}", status.code());
            }

            println!("  Vite build successful!");
        } else {
            // Use pure Rust OXC bundler
            println!("Stage 3: Building web application with OXC bundler...");

            let entry_point = if main_tsx.exists() { main_tsx } else { index_tsx };
            let public_dir = build_dir.join("public");

            let config = WebBundleConfig {
                project_root: build_dir.clone(),
                src_dir: src_dir.clone(),
                out_dir: dist_dir.clone(),
                index_html: index_html.clone(),
                entry_point,
                public_dir: if public_dir.exists() { Some(public_dir) } else { None },
                base_path: "/".to_string(),
                minify: true,
                externals: web_bundler::default_react_externals(),
                bundle_node_modules: false,
            };

            let result = web_bundler::bundle_web_app(&config)
                .with_context(|| "Web bundler failed")?;

            println!("  Bundle: {}", result.js_bundle.file_name().unwrap().to_string_lossy());
            println!("  Hash: {}", result.bundle_hash);
            println!("  Assets: {} files", result.assets.len());
            println!("  OXC build successful!");
        }
    } else if has_tsconfig {
        // =====================================================================
        // CLI TOOL BUILD (TypeScript): Simple transpilation (no bundling)
        // =====================================================================
        println!("Stage 3: Compiling TypeScript with OXC (pure Rust)...");

        // Compile TypeScript with OXC (synchronous)
        let compiled = rolldown_bundler::compile_typescript(&src_dir, &dist_dir)
            .with_context(|| "OXC TypeScript compilation failed")?;

        println!("  Compiled {} files with OXC", compiled.len());

        // Generate declaration files using our stub generator
        let declarations = generate_declarations(&src_dir, &dist_dir)
            .with_context(|| "Declaration file generation failed")?;

        println!("  Generated {} declaration files", declarations.len());
        println!("  TypeScript compilation successful");
    } else {
        // =====================================================================
        // CLI TOOL BUILD (JavaScript): Copy source files directly
        // =====================================================================
        println!("  No tsconfig.json found — JavaScript mode (skipping TypeScript compilation)");
        println!("Stage 3: Copying JavaScript source files...");

        let copied = copy_js_source_files(&src_dir, &dist_dir)
            .with_context(|| "JavaScript source file copy failed")?;

        println!("  Copied {} files", copied.len());
        println!("  JavaScript source copy successful");
    }

    // =========================================================================
    // STAGE 4: Copy built output
    // =========================================================================
    println!();
    println!("Stage 4: Copying build output...");

    let dist_dir = build_dir.join("dist");
    if !dist_dir.exists() {
        anyhow::bail!("dist/ directory not found after compilation");
    }

    let output_dist = lib_dir.join("dist");
    copy_dir_recursive(&dist_dir, &output_dist)?;

    // Symlink node_modules for runtime
    let output_node_modules = lib_dir.join("node_modules");
    std::os::unix::fs::symlink(node_modules_dir.join("node_modules"), &output_node_modules)?;

    // =========================================================================
    // STAGE 5: Create wrapper script
    // =========================================================================
    if let (Some(cli_entry), Some(bin_name)) = (&args.cli_entry, &args.bin_name) {
        println!();
        println!("Stage 5: Creating wrapper script...");

        let cli_path = output_dist.join(cli_entry);
        if !cli_path.exists() {
            anyhow::bail!("CLI entry point not found: {}", cli_path.display());
        }

        let wrapper_path = bin_dir.join(bin_name);
        let wrapper_content = format!(
            "#!{}\nexec {} {} \"$@\"\n",
            "/usr/bin/env bash",
            args.node_bin.display(),
            cli_path.display()
        );
        fs::write(&wrapper_path, wrapper_content)?;
        fs::set_permissions(&wrapper_path, fs::Permissions::from_mode(0o755))?;

        println!("  Created: {}", wrapper_path.display());
    }

    // =========================================================================
    // Cleanup
    // =========================================================================
    println!();
    println!("Stage 6: Cleaning up...");
    fs::remove_dir_all(&build_dir)?;

    println!();
    println!("Done!");
    println!("  Output: {}", args.output.display());

    Ok(())
}

/// Set up a workspace dependency in the build environment
fn setup_workspace_dep(
    build_dir: &Path,
    build_node_modules: &Path,
    dep_info: &WorkspaceDepInfo,
) -> Result<()> {
    println!(
        "  Setting up workspace dep: {} -> {}",
        dep_info.name,
        dep_info.built_path.display()
    );

    // Create scope directory if needed
    let dep_target = if dep_info.name.starts_with('@') {
        let parts: Vec<&str> = dep_info.name.splitn(2, '/').collect();
        if parts.len() == 2 {
            let scope_dir = build_node_modules.join(parts[0]);
            fs::create_dir_all(&scope_dir)?;
            scope_dir.join(parts[1])
        } else {
            build_node_modules.join(&dep_info.name)
        }
    } else {
        build_node_modules.join(&dep_info.name)
    };

    // Remove existing and symlink to the dependency
    if dep_target.symlink_metadata().is_ok() {
        fs::remove_file(&dep_target).or_else(|_| fs::remove_dir_all(&dep_target))?;
    }
    std::os::unix::fs::symlink(&dep_info.built_path, &dep_target)?;

    // Set up sibling directory for tsconfig project references
    let sibling_name = dep_info
        .name
        .split('/')
        .last()
        .unwrap_or(&dep_info.name);
    let sibling_dir = build_dir.parent().unwrap().join(sibling_name);

    if !sibling_dir.exists() {
        fs::create_dir_all(&sibling_dir)?;

        // Copy dist from dependency
        if dep_info.built_path.join("dist").exists() {
            copy_dir_recursive(&dep_info.built_path.join("dist"), &sibling_dir.join("dist"))?;
        }

        // Copy package.json
        if dep_info.built_path.join("package.json").exists() {
            fs::copy(
                dep_info.built_path.join("package.json"),
                sibling_dir.join("package.json"),
            )?;
        }

        // Copy tsconfig.json from original source (needed for project references)
        if let Some(source_path) = &dep_info.source_path {
            let tsconfig_src = source_path.join("tsconfig.json");
            if tsconfig_src.exists() {
                fs::copy(&tsconfig_src, sibling_dir.join("tsconfig.json"))?;
            }
        }
    }

    Ok(())
}

/// Build a workspace package from source (used by --workspace-src)
/// Uses OXC for TypeScript compilation (pure Rust, no Node.js dependency)
fn build_workspace_package(
    name: &str,
    manifest: &Path,
    src: &Path,
    output: &Path,
    node_bin: &Path,
    parent_tsconfig: Option<&Path>,
    existing_workspace_deps: &[(String, PathBuf)],
) -> Result<()> {
    println!("    Building workspace package: {}", name);

    let lib_dir = output.join("lib");
    fs::create_dir_all(&lib_dir)?;

    // Build node_modules for this package
    let node_modules_dir = lib_dir.join("node_modules_build");
    run_build(BuildArgs {
        manifest: manifest.to_path_buf(),
        output: node_modules_dir.clone(),
        node_bin: node_bin.to_path_buf(),
    })?;

    // Set up build directory
    let build_dir = lib_dir.join("build_workspace");
    fs::create_dir_all(&build_dir)?;

    // Copy source to build directory
    copy_dir_recursive(src, &build_dir)?;

    // Symlink node_modules
    let build_node_modules = build_dir.join("node_modules");
    if build_node_modules.exists() {
        fs::remove_dir_all(&build_node_modules)?;
    }
    std::os::unix::fs::symlink(node_modules_dir.join("node_modules"), &build_node_modules)?;

    // Set up parent tsconfig if specified
    if let Some(parent_tsconfig) = parent_tsconfig {
        let parent_dest = build_dir.parent().unwrap().join("tsconfig.json");
        fs::copy(parent_tsconfig, &parent_dest)?;
    }

    // Set up existing workspace dependencies
    for (dep_name, dep_path) in existing_workspace_deps {
        let dep_target = if dep_name.starts_with('@') {
            let parts: Vec<&str> = dep_name.splitn(2, '/').collect();
            if parts.len() == 2 {
                let scope_dir = build_node_modules.join(parts[0]);
                fs::create_dir_all(&scope_dir)?;
                scope_dir.join(parts[1])
            } else {
                build_node_modules.join(dep_name)
            }
        } else {
            build_node_modules.join(dep_name)
        };

        if dep_target.symlink_metadata().is_ok() {
            fs::remove_file(&dep_target).or_else(|_| fs::remove_dir_all(&dep_target))?;
        }
        std::os::unix::fs::symlink(dep_path, &dep_target)?;
    }

    // Compile or copy source files depending on project type
    let ws_src_dir = build_dir.join("src");
    let ws_dist_dir = build_dir.join("dist");

    fs::create_dir_all(&ws_dist_dir)?;

    let ws_has_tsconfig = build_dir.join("tsconfig.json").exists();

    if ws_has_tsconfig {
        // TypeScript mode: compile with OXC (synchronous)
        rolldown_bundler::compile_typescript(&ws_src_dir, &ws_dist_dir)
            .with_context(|| format!("OXC compilation failed for workspace package {}", name))?;

        generate_declarations(&ws_src_dir, &ws_dist_dir)
            .with_context(|| format!("Declaration generation failed for workspace package {}", name))?;
    } else {
        // JavaScript mode: copy source files directly
        println!("    No tsconfig.json — JavaScript mode for {}", name);
        copy_js_source_files(&ws_src_dir, &ws_dist_dir)
            .with_context(|| format!("JS source copy failed for workspace package {}", name))?;
    }

    // Copy dist to output
    let dist_dir = build_dir.join("dist");
    if !dist_dir.exists() {
        anyhow::bail!(
            "dist/ directory not found after compilation for workspace package {}",
            name
        );
    }

    let output_dist = lib_dir.join("dist");
    copy_dir_recursive(&dist_dir, &output_dist)?;

    // Copy package.json for module resolution
    let package_json_src = src.join("package.json");
    if package_json_src.exists() {
        fs::copy(&package_json_src, lib_dir.join("package.json"))?;
    }

    // Symlink node_modules for runtime
    let output_node_modules = lib_dir.join("node_modules");
    std::os::unix::fs::symlink(node_modules_dir.join("node_modules"), &output_node_modules)?;

    // Cleanup
    fs::remove_dir_all(&build_dir)?;

    println!("    Workspace package {} built successfully", name);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_has_tsconfig_true_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("tsconfig.json"), "{}").unwrap();

        let has_tsconfig = tmp.path().join("tsconfig.json").exists();
        assert!(has_tsconfig, "has_tsconfig should be true when tsconfig.json exists");
    }

    #[test]
    fn test_has_tsconfig_false_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        // No tsconfig.json created

        let has_tsconfig = tmp.path().join("tsconfig.json").exists();
        assert!(!has_tsconfig, "has_tsconfig should be false when tsconfig.json is missing");
    }

    #[test]
    fn test_copy_js_source_files_copies_expected_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        let out_dir = tmp.path().join("out");
        fs::create_dir_all(&src_dir).unwrap();

        // Create files with various extensions
        fs::write(src_dir.join("index.js"), "export default 1;").unwrap();
        fs::write(src_dir.join("utils.mjs"), "export const a = 1;").unwrap();
        fs::write(src_dir.join("config.cjs"), "module.exports = {};").unwrap();
        fs::write(src_dir.join("data.json"), "{}").unwrap();
        // These should NOT be copied
        fs::write(src_dir.join("style.css"), "body {}").unwrap();
        fs::write(src_dir.join("readme.md"), "# hi").unwrap();
        fs::write(src_dir.join("main.ts"), "const x: number = 1;").unwrap();

        let copied = copy_js_source_files(&src_dir, &out_dir).unwrap();

        assert_eq!(copied.len(), 4, "Should copy exactly 4 files (.js, .mjs, .cjs, .json)");
        assert!(out_dir.join("index.js").exists(), "index.js should be copied");
        assert!(out_dir.join("utils.mjs").exists(), "utils.mjs should be copied");
        assert!(out_dir.join("config.cjs").exists(), "config.cjs should be copied");
        assert!(out_dir.join("data.json").exists(), "data.json should be copied");
        assert!(!out_dir.join("style.css").exists(), "style.css should NOT be copied");
        assert!(!out_dir.join("readme.md").exists(), "readme.md should NOT be copied");
        assert!(!out_dir.join("main.ts").exists(), "main.ts should NOT be copied");
    }
}
