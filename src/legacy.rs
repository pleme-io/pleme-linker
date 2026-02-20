//! Legacy commands (to be removed after migration)

use anyhow::Result;

use crate::build::run_build;
use crate::cli::{BuildArgs, BuildLibrariesArgs, BuildNodeModulesArgs, LinkArgs};

/// Run the legacy link command
pub fn run_link(_args: LinkArgs) -> Result<()> {
    println!("Legacy 'link' command - please migrate to new workflow");
    println!("See: pleme-linker --help");
    Ok(())
}

/// Run the legacy build-libraries command
pub fn run_build_libraries(_args: BuildLibrariesArgs) -> Result<()> {
    println!("Legacy 'build-libraries' command");
    println!("Local libraries are now handled via workspace support");
    Ok(())
}

/// Run the legacy build-node-modules command
pub fn run_build_node_modules(args: BuildNodeModulesArgs) -> Result<()> {
    // Redirect to new build command
    run_build(BuildArgs {
        manifest: args.manifest,
        output: args.output,
        node_bin: args.node_bin,
    })
}
