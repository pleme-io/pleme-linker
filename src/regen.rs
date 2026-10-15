//! Regeneration commands for web projects
//!
//! This module handles frontend deps.nix generation. Cargo.nix regeneration
//! for backend services is handled separately by the deployment toolchain.

use anyhow::Result;

use crate::cli::{CargoUpdateArgs, RegenArgs, ResolveArgs};
use crate::resolve::run_resolve;

/// Run the regen command
///
/// This only regenerates frontend deps.nix. Backend Cargo.nix is handled
/// separately by the deployment toolchain.
pub async fn run_regen(args: RegenArgs) -> Result<()> {
    println!("pleme-linker regen: Regenerating web project dependencies");
    println!("  Project: {}", args.project_root.display());
    println!();

    // Run resolve to generate deps.nix for frontend
    println!("Resolving npm dependencies...");
    let deps_output = args.project_root.join("deps.nix");

    run_resolve(ResolveArgs {
        project: args.project_root.clone(),
        output: Some(deps_output),
        include_dev: true,
        registry: "https://registry.npmjs.org".to_string(),
        platform: "linux".to_string(),
    })
    .await?;

    println!();
    println!("Done!");
    println!();
    println!("Next steps:");
    println!("   1. Review the changes: git diff");
    println!("   2. Commit the generated files: git add -A && git commit");
    println!("   3. Run the release: nix run .#release:<product>:<service>");

    Ok(())
}

/// Run the cargo-update command
///
/// DEPRECATED: Use cargo update directly in your service directory.
pub fn run_cargo_update(args: CargoUpdateArgs) -> Result<()> {
    println!("⚠️  pleme-linker cargo-update is DEPRECATED");
    println!();
    println!("Use cargo update directly in your service directory, then regenerate:");
    println!();
    println!("   cd <service-dir> && cargo update && crate2nix generate");

    // Keep args to avoid unused warning
    let _ = args;

    Ok(())
}
