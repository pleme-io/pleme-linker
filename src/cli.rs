//! CLI argument definitions for pleme-linker

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "pleme-linker")]
#[command(about = "Nix-native JavaScript package manager")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Resolve dependencies and generate deps.nix
    Resolve(ResolveArgs),

    /// Build node_modules from fetched tarballs
    Build(BuildArgs),

    /// Build a complete TypeScript project (node_modules + tsc + wrapper)
    BuildProject(BuildProjectArgs),

    /// Regenerate deps.nix and Cargo.nix for web projects
    Regen(RegenArgs),

    /// Update Cargo.lock for web-server
    CargoUpdate(CargoUpdateArgs),

    /// Build a TypeScript library (runs tsdown to produce dist/)
    /// Used by Nix to build @pleme/* libraries as derivations
    BuildLibrary(BuildLibraryArgs),

    // Legacy commands (to be removed after migration)
    /// [Legacy] Link @pleme libraries into node_modules
    Link(LinkArgs),
    /// [Legacy] Build @pleme libraries
    BuildLibraries(BuildLibrariesArgs),
    /// [Legacy] Build node_modules from manifest
    BuildNodeModules(BuildNodeModulesArgs),
}

#[derive(Parser, Debug)]
pub struct ResolveArgs {
    /// Path to project root (containing package.json)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Output path for deps.nix
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Include devDependencies
    #[arg(long, default_value_t = true)]
    pub include_dev: bool,

    /// npm registry URL
    #[arg(long, default_value = "https://registry.npmjs.org")]
    pub registry: String,

    /// Target platform (linux, darwin)
    #[arg(long, default_value = "linux")]
    pub platform: String,
}

#[derive(Parser, Debug, Clone)]
pub struct BuildArgs {
    /// Path to manifest JSON file
    #[arg(long)]
    pub manifest: PathBuf,

    /// Output directory for node_modules
    #[arg(long)]
    pub output: PathBuf,

    /// Path to Node.js binary (for running postinstall scripts)
    #[arg(long)]
    pub node_bin: PathBuf,
}

/// Parse workspace dependency argument: "name=path"
pub fn parse_workspace_dep(s: &str) -> Result<(String, PathBuf), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid workspace-dep format: '{}'. Expected 'name=path'",
            s
        ));
    }
    Ok((parts[0].to_string(), PathBuf::from(parts[1])))
}

/// Workspace source for building from source
#[derive(Debug, Clone)]
pub struct WorkspaceSrc {
    pub name: String,
    pub manifest: PathBuf,
    pub src: PathBuf,
}

/// Parse workspace source argument: "name=manifest=srcPath"
pub fn parse_workspace_src(s: &str) -> Result<WorkspaceSrc, String> {
    let parts: Vec<&str> = s.splitn(3, '=').collect();
    if parts.len() != 3 {
        return Err(format!(
            "Invalid workspace-src format: '{}'. Expected 'name=manifest=srcPath'",
            s
        ));
    }
    Ok(WorkspaceSrc {
        name: parts[0].to_string(),
        manifest: PathBuf::from(parts[1]),
        src: PathBuf::from(parts[2]),
    })
}

#[derive(Parser, Debug)]
pub struct BuildProjectArgs {
    /// Path to manifest JSON file (npm packages + workspace packages)
    #[arg(long)]
    pub manifest: PathBuf,

    /// Project source directory (containing package.json, tsconfig.json, src/)
    #[arg(long)]
    pub project: PathBuf,

    /// Output directory for the built project
    #[arg(long)]
    pub output: PathBuf,

    /// Path to Node.js binary
    #[arg(long)]
    pub node_bin: PathBuf,

    /// CLI entry point (relative to dist/, e.g., "cli.js")
    #[arg(long)]
    pub cli_entry: Option<String>,

    /// Name for the wrapper binary
    #[arg(long)]
    pub bin_name: Option<String>,

    /// Path to parent tsconfig.json (if project extends it)
    #[arg(long)]
    pub parent_tsconfig: Option<PathBuf>,

    /// Pre-built workspace dependency: name=path (can be specified multiple times)
    #[arg(long, value_parser = parse_workspace_dep)]
    pub workspace_dep: Vec<(String, PathBuf)>,

    /// Workspace source: name=manifest=srcPath (build from source)
    #[arg(long, value_parser = parse_workspace_src)]
    pub workspace_src: Vec<WorkspaceSrc>,

    /// Use Vite for web app bundling instead of the pure Rust OXC bundler
    /// When set, shells out to `npx vite build` instead of using the built-in bundler
    #[arg(long, default_value_t = false)]
    pub use_vite: bool,
}

#[derive(Parser, Debug)]
pub struct RegenArgs {
    /// Path to project root (containing package.json and web-server/)
    #[arg(long)]
    pub project_root: PathBuf,

    /// Path to crate2nix binary
    #[arg(long)]
    pub crate2nix: PathBuf,
}

#[derive(Parser, Debug)]
pub struct CargoUpdateArgs {
    /// Path to project root (containing web-server/)
    #[arg(long)]
    pub project_root: PathBuf,

    /// Path to cargo binary
    #[arg(long)]
    pub cargo: PathBuf,
}

/// Arguments for build-library command
/// Builds a TypeScript library using tsdown (produces dist/)
#[derive(Parser, Debug)]
pub struct BuildLibraryArgs {
    /// Path to manifest JSON file (npm packages from deps.nix)
    #[arg(long)]
    pub manifest: PathBuf,

    /// Path to library source directory (containing package.json, src/, tsdown.config.ts)
    #[arg(long)]
    pub src: PathBuf,

    /// Output directory for the built library
    #[arg(long)]
    pub output: PathBuf,

    /// Path to Node.js binary
    #[arg(long)]
    pub node_bin: PathBuf,
}

// Legacy command args
#[derive(Parser, Debug)]
pub struct LinkArgs {
    #[arg(long)]
    pub libraries_dir: PathBuf,
    #[arg(long)]
    pub node_modules: PathBuf,
}

#[derive(Parser, Debug)]
pub struct BuildLibrariesArgs {
    #[arg(long)]
    pub libraries_dir: PathBuf,
    #[arg(long)]
    pub node_bin: PathBuf,
}

#[derive(Parser, Debug)]
pub struct BuildNodeModulesArgs {
    #[arg(long)]
    pub manifest: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long)]
    pub node_bin: PathBuf,
}
