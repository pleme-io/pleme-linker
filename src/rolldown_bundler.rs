//! OXC-based TypeScript/JavaScript compilation
//!
//! Uses OXC (Oxidation Compiler) - the same Rust toolchain that powers Rolldown.
//! Pure Rust implementation: no Node.js dependencies for compilation.
//!
//! Architecture:
//! - Parse TypeScript/JavaScript with OXC parser
//! - Transform TypeScript to JavaScript with OXC transformer
//! - Minify with OXC minifier
//! - Generate output with OXC codegen

use anyhow::{Context, Result};
use std::fs;
use std::mem;
use std::path::{Path, PathBuf};

use oxc::codegen::{CodegenOptions, CodegenReturn};
use oxc::diagnostics::OxcDiagnostic;
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, TypeScriptOptions};
use oxc::CompilerInterface;

/// Custom compiler for CLI tools that strips TypeScript types
/// Similar to WebCompiler but without JSX configuration
struct CliCompiler {
    transform_options: TransformOptions,
    printed: String,
    errors: Vec<OxcDiagnostic>,
}

impl CliCompiler {
    fn new() -> Self {
        // Configure TypeScript transform to strip all type information
        let typescript = TypeScriptOptions {
            only_remove_type_imports: false,
            allow_namespaces: true,
            allow_declare_fields: true,
            remove_class_fields_without_initializer: false,
            ..Default::default()
        };

        let transform_options = TransformOptions {
            typescript,
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

impl CompilerInterface for CliCompiler {
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

/// Compile a single TypeScript/JavaScript file using OXC
pub fn compile_file(source_path: &Path) -> Result<String> {
    let source_text = fs::read_to_string(source_path)
        .with_context(|| format!("Failed to read source file: {}", source_path.display()))?;

    let source_type = SourceType::from_path(source_path)
        .map_err(|e| anyhow::anyhow!("Failed to determine source type: {:?}", e))?;

    compile_source(&source_text, source_type, source_path)
}

/// Compile source code string using OXC
pub fn compile_source(
    source_text: &str,
    source_type: SourceType,
    source_path: &Path,
) -> Result<String> {
    let mut compiler = CliCompiler::new();
    match compiler.execute(source_text, source_type, source_path) {
        Ok(output) => Ok(output),
        Err(errors) => {
            let error_messages: Vec<String> = errors
                .iter()
                .map(|e| format!("{:?}", e))
                .collect();
            anyhow::bail!("Compilation errors:\n{}", error_messages.join("\n"))
        }
    }
}

/// Compile all TypeScript files in a directory to JavaScript
///
/// This is the main entry point called by build_project.rs.
/// Note: This is synchronous since OXC is fully synchronous.
pub fn compile_typescript(src_dir: &Path, out_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut outputs = Vec::new();

    // Create output directory
    fs::create_dir_all(out_dir)?;

    // Find all TypeScript/JavaScript files
    for entry in walkdir::WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file()
                && path
                    .extension()
                    .map_or(false, |ext| ext == "ts" || ext == "tsx" || ext == "js" || ext == "jsx")
        })
    {
        let source_path = entry.path();
        let relative_path = source_path
            .strip_prefix(src_dir)
            .unwrap_or(source_path);

        // Change extension to .js
        let mut output_path = out_dir.join(relative_path);
        output_path.set_extension("js");

        // Create parent directories
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Compile the file
        println!("    Compiling: {}", relative_path.display());
        let output = compile_file(source_path)
            .with_context(|| format!("Failed to compile {}", source_path.display()))?;

        // Write output
        fs::write(&output_path, &output)?;
        outputs.push(output_path);
    }

    println!("  Compiled {} files with OXC", outputs.len());
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_simple_typescript() {
        let source = "const x: number = 42; console.log(x);";
        let source_type = SourceType::ts();

        let result = compile_source(source, source_type, Path::new("test.ts")).unwrap();
        assert!(result.contains("console.log"));
        // TypeScript type annotation should be stripped
        assert!(!result.contains(": number"));
    }

    #[test]
    fn test_compile_tsx() {
        let source = "const App = () => <div>Hello</div>;";
        let source_type = SourceType::tsx();

        let result = compile_source(source, source_type, Path::new("test.tsx")).unwrap();
        // JSX should be transformed
        assert!(!result.contains("<div>"));
    }
}
