//! TypeScript library building (produces dist/ via tsdown)
//!
//! This command builds @pleme/* TypeScript libraries for use by dependent projects.
//! It's designed to be called by Nix derivations to produce reproducible library builds.
//!
//! Flow:
//! 1. Build node_modules from pre-fetched tarballs (via manifest JSON)
//! 2. Run tsdown to compile TypeScript to dist/
//! 3. Copy the built library to output directory
//!
//! Unlike build-project (which handles full apps), this focuses on library builds
//! that produce a package with dist/ for consumption by other projects.

use anyhow::{Context, Result};
use std::fs;

use crate::build::run_build;
use crate::cli::{BuildArgs, BuildLibraryArgs};
use crate::utils::copy_dir_recursive;

/// Run the build-library command
pub fn run_build_library(args: BuildLibraryArgs) -> Result<()> {
    println!("pleme-linker build-library: Building TypeScript library");
    println!("  Source:   {}", args.src.display());
    println!("  Output:   {}", args.output.display());
    println!("  Manifest: {}", args.manifest.display());
    println!();

    // Create output directory
    fs::create_dir_all(&args.output)?;

    // =========================================================================
    // STAGE 1: Build node_modules from manifest
    // =========================================================================
    println!("Stage 1: Building node_modules...");

    let node_modules_build = args.output.join("node_modules_build");
    run_build(BuildArgs {
        manifest: args.manifest.clone(),
        output: node_modules_build.clone(),
        node_bin: args.node_bin.clone(),
    })?;

    // =========================================================================
    // STAGE 2: Set up build directory
    // =========================================================================
    println!();
    println!("Stage 2: Setting up build environment...");

    let build_dir = args.output.join("build_workspace");
    fs::create_dir_all(&build_dir)?;

    // Copy library source to build directory
    copy_dir_recursive(&args.src, &build_dir)?;

    // Symlink node_modules into build directory
    let build_node_modules = build_dir.join("node_modules");
    if build_node_modules.exists() {
        fs::remove_dir_all(&build_node_modules)?;
    }
    std::os::unix::fs::symlink(
        node_modules_build.join("node_modules"),
        &build_node_modules,
    )?;

    println!("  Build directory ready");

    // =========================================================================
    // STAGE 3: Run tsdown to build dist/
    // =========================================================================
    println!();
    println!("Stage 3: Running tsdown...");

    // tsdown is the build tool for @pleme/* libraries
    // It's configured via tsdown.config.ts in each library
    //
    // The .bin/tsdown is a shell script that wraps the actual JS entry point.
    // In Nix, we need to execute it as a shell script, not pass it to Node.
    let tsdown_bin = build_node_modules.join(".bin/tsdown");

    if !tsdown_bin.exists() {
        anyhow::bail!(
            "tsdown not found in node_modules/.bin. Ensure it's in devDependencies.\n\
             Expected at: {}",
            tsdown_bin.display()
        );
    }

    // Execute the bin script directly (it's a shell wrapper that invokes node)
    // The script has a shebang and handles invoking node itself
    let status = std::process::Command::new(&tsdown_bin)
        .current_dir(&build_dir)
        .env("NODE_ENV", "production")
        .env("PATH", args.node_bin.parent().unwrap_or(&args.node_bin))
        .status()
        .with_context(|| "Failed to run tsdown")?;

    if !status.success() {
        anyhow::bail!("tsdown failed with exit code: {:?}", status.code());
    }

    println!("  tsdown completed successfully");

    // =========================================================================
    // STAGE 4: Copy built output
    // =========================================================================
    println!();
    println!("Stage 4: Copying build output...");

    let dist_dir = build_dir.join("dist");
    if !dist_dir.exists() {
        anyhow::bail!(
            "dist/ directory not found after tsdown build.\n\
             Expected at: {}\n\
             Check tsdown.config.ts for output configuration.",
            dist_dir.display()
        );
    }

    // Copy to final output location
    let output_dist = args.output.join("dist");
    copy_dir_recursive(&dist_dir, &output_dist)?;

    // Copy package.json (needed for module resolution)
    let package_json_src = build_dir.join("package.json");
    if package_json_src.exists() {
        fs::copy(&package_json_src, args.output.join("package.json"))?;
    }

    // Copy other important files if they exist
    for file in &["README.md", "tsconfig.json", "tsconfig.build.json"] {
        let src_file = build_dir.join(file);
        if src_file.exists() {
            fs::copy(&src_file, args.output.join(file))?;
        }
    }

    // =========================================================================
    // STAGE 5: Cleanup
    // =========================================================================
    println!();
    println!("Stage 5: Cleaning up...");

    // Remove build directory (keep only final output)
    fs::remove_dir_all(&build_dir)?;
    fs::remove_dir_all(&node_modules_build)?;

    println!();
    println!("Done!");
    println!("  Output: {}", args.output.display());
    println!("  dist/: {}", output_dist.display());

    Ok(())
}
