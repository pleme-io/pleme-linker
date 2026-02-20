/*!
 * pleme-linker - Nix-Native JavaScript Package Manager
 *
 * A Rust-based package manager designed from the ground up for Nix builds.
 * Instead of fighting npm/pnpm/yarn in the Nix sandbox, we own the entire pipeline.
 *
 * Key insight: The npm registry is just HTTP. We don't need npm/pnpm/yarn at all.
 *
 * Commands:
 *   resolve       - Query npm registry, resolve versions, generate deps.nix
 *   build         - Build node_modules from fetched tarballs (no network needed)
 *   build-project - Build complete TypeScript project (node_modules + tsc + wrapper)
 *   build-library - Build TypeScript library via tsdown (produces dist/)
 *   regen         - Regenerate deps.nix and Cargo.nix for web projects
 *   cargo-update  - Update Cargo.lock for web-server
 *
 * See DESIGN.md for full architecture documentation.
 */

mod build;
mod build_library;
mod build_project;
mod cli;
mod legacy;
mod regen;
mod resolve;
mod rolldown_bundler;
mod swc_compiler;
mod types;
mod utils;
mod web_bundler;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Resolve(args) => resolve::run_resolve(args).await,
        Commands::Build(args) => build::run_build(args),
        Commands::BuildProject(args) => build_project::run_build_project(args),
        Commands::BuildLibrary(args) => build_library::run_build_library(args),
        Commands::Regen(args) => regen::run_regen(args).await,
        Commands::CargoUpdate(args) => regen::run_cargo_update(args),
        // Legacy commands
        Commands::Link(args) => legacy::run_link(args),
        Commands::BuildLibraries(args) => legacy::run_build_libraries(args),
        Commands::BuildNodeModules(args) => legacy::run_build_node_modules(args),
    }
}
