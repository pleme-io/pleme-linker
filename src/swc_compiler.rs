//! Pure Rust TypeScript compilation using SWC
//!
//! This module provides native TypeScript → JavaScript compilation
//! without shelling out to tsc or esbuild.
//!
//! **Status**: Currently scaffolded, defaults to tsc until SWC version
//! compatibility is resolved. The architecture is in place for full
//! Nix-native builds.
//!
//! Architecture for Nix-native builds:
//! - Each @pleme/* library is compiled to its own derivation
//! - These derivations become dependencies for product builds
//! - Final output is a pure Nix derivation with bundled JS
//!
//! TODO: Implement native SWC compilation when version compatibility
//! with serde is resolved (swc_config uses deprecated serde::__private).

use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// ES Version targets (for future native compilation)
#[derive(Debug, Clone, Copy, Default)]
pub enum EsTarget {
    Es5,
    Es2015,
    Es2016,
    Es2017,
    Es2018,
    Es2019,
    #[default]
    Es2020,
    Es2021,
    Es2022,
    EsNext,
}

/// Options for TypeScript compilation
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    /// Target ES version (default: ES2020)
    pub target: EsTarget,
    /// Enable JSX transformation
    pub jsx: bool,
    /// Use React automatic runtime (React 17+)
    pub jsx_automatic_runtime: bool,
    /// Generate source maps
    pub source_maps: bool,
    /// Module format (ESM)
    pub module: bool,
}

/// Result of compiling a TypeScript file
pub struct CompileResult {
    /// Generated JavaScript code
    pub code: String,
    /// Source map (if enabled)
    pub source_map: Option<String>,
}

/// Compile a single TypeScript/TSX file to JavaScript
///
/// Note: Native SWC compilation is not yet available. Use --use-tsc flag
/// to compile with external tsc, or wait for native compilation support.
pub fn compile_file(_source: &str, _filename: &str, _options: &CompileOptions) -> Result<CompileResult> {
    bail!(
        "Native SWC compilation not yet available. Use --use-tsc flag for TypeScript compilation.\n\
         Native compilation will be enabled once SWC version compatibility is resolved."
    )
}

/// Compile a TypeScript project directory
///
/// Note: Native compilation not yet available. This function will copy
/// source files and defer to tsc for actual compilation.
pub fn compile_project(
    src_dir: &Path,
    out_dir: &Path,
    _options: &CompileOptions,
) -> Result<Vec<PathBuf>> {
    // For now, just copy TypeScript files to output
    // The actual compilation happens via tsc in build_project.rs when --use-tsc is set
    let mut copied_files = vec![];

    copy_ts_files(src_dir, src_dir, out_dir, &mut copied_files)?;

    Ok(copied_files)
}

fn copy_ts_files(
    root_src: &Path,
    current_src: &Path,
    out_dir: &Path,
    copied_files: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(current_src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        // Skip node_modules, test files, and hidden directories
        if file_name_str.starts_with('.')
            || file_name_str == "node_modules"
            || file_name_str.ends_with(".test.ts")
            || file_name_str.ends_with(".test.tsx")
            || file_name_str.ends_with(".spec.ts")
            || file_name_str.ends_with(".spec.tsx")
            || file_name_str == "__tests__"
        {
            continue;
        }

        if path.is_dir() {
            let relative_path = path.strip_prefix(root_src)?;
            let out_subdir = out_dir.join(relative_path);
            fs::create_dir_all(&out_subdir)?;

            copy_ts_files(root_src, &path, out_dir, copied_files)?;
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            if matches!(ext, "ts" | "tsx" | "js" | "jsx" | "json" | "css") {
                let relative_path = path.strip_prefix(root_src)?;
                let out_path = out_dir.join(relative_path);
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&path, &out_path)?;
                copied_files.push(out_path);
            }
        }
    }

    Ok(())
}

/// Generate TypeScript declaration files (.d.ts)
///
/// This implementation extracts exports and generates basic declaration stubs.
pub fn generate_declarations(
    src_dir: &Path,
    out_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let mut declaration_files = vec![];

    generate_declarations_for_directory(src_dir, src_dir, out_dir, &mut declaration_files)?;

    Ok(declaration_files)
}

fn generate_declarations_for_directory(
    root_src: &Path,
    current_src: &Path,
    out_dir: &Path,
    declaration_files: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(current_src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        // Skip node_modules, test files, and hidden directories
        if file_name_str.starts_with('.')
            || file_name_str == "node_modules"
            || file_name_str.ends_with(".test.ts")
            || file_name_str.ends_with(".test.tsx")
            || file_name_str.ends_with(".spec.ts")
            || file_name_str.ends_with(".spec.tsx")
            || file_name_str == "__tests__"
        {
            continue;
        }

        if path.is_dir() {
            let relative_path = path.strip_prefix(root_src)?;
            let out_subdir = out_dir.join(relative_path);
            fs::create_dir_all(&out_subdir)?;

            generate_declarations_for_directory(root_src, &path, out_dir, declaration_files)?;
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            // Copy existing .d.ts files
            if file_name_str.ends_with(".d.ts") {
                let relative_path = path.strip_prefix(root_src)?;
                let out_path = out_dir.join(relative_path);
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&path, &out_path)?;
                declaration_files.push(out_path);
            } else if matches!(ext, "ts" | "tsx") {
                // Generate declaration file from source
                let relative_path = path.strip_prefix(root_src)?;
                let mut out_path = out_dir.join(relative_path);
                out_path.set_extension("d.ts");

                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                // Read source to extract exports
                let source = fs::read_to_string(&path)?;
                let decl_content = extract_exports_for_declaration(&source);

                fs::write(&out_path, decl_content)?;
                declaration_files.push(out_path);
            }
        }
    }

    Ok(())
}

/// Extract exports and generate basic declaration content
fn extract_exports_for_declaration(source: &str) -> String {
    let mut declarations = vec![];

    for line in source.lines() {
        let trimmed = line.trim();

        // Export type/interface declarations - pass through
        if trimmed.starts_with("export type ") || trimmed.starts_with("export interface ") {
            declarations.push(line.to_string());
            continue;
        }

        // Named exports with types
        if trimmed.starts_with("export const ") {
            if let Some(name_end) = trimmed.find(':') {
                let name_part = &trimmed[13..name_end].trim();
                let type_start = name_end + 1;
                if let Some(eq_pos) = trimmed[type_start..].find('=') {
                    let type_part = trimmed[type_start..type_start + eq_pos].trim();
                    declarations.push(format!("export declare const {}: {};", name_part, type_part));
                }
            } else if let Some(eq_pos) = trimmed.find('=') {
                let name_part = &trimmed[13..eq_pos].trim();
                declarations.push(format!("export declare const {}: any;", name_part));
            }
            continue;
        }

        // Export function declarations
        if trimmed.starts_with("export function ") || trimmed.starts_with("export async function ") {
            let fn_start = if trimmed.starts_with("export async") { 22 } else { 16 };
            if let Some(paren_pos) = trimmed.find('(') {
                let fn_name = &trimmed[fn_start..paren_pos].trim();
                declarations.push(format!("export declare function {}(...args: any[]): any;", fn_name));
            }
            continue;
        }

        // Re-exports
        if trimmed.starts_with("export {") || trimmed.starts_with("export * from") {
            declarations.push(line.to_string());
            continue;
        }
    }

    if declarations.is_empty() {
        "export {};\n".to_string()
    } else {
        declarations.join("\n") + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_exports() {
        let source = r#"
            export type Foo = string;
            export interface Bar { name: string; }
            export const greeting: string = "hello";
            export function greet(name: string): string { return name; }
            export * from './other';
        "#;

        let decl = extract_exports_for_declaration(source);
        assert!(decl.contains("export type Foo"));
        assert!(decl.contains("export interface Bar"));
        assert!(decl.contains("export declare const greeting"));
        assert!(decl.contains("export declare function greet"));
        assert!(decl.contains("export * from"));
    }
}
