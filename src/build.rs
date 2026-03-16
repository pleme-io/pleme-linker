//! Build node_modules from fetched tarballs

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::cli::BuildArgs;
use crate::types::BuildManifest;
use crate::utils::diff_paths;

/// Run the build command
pub fn run_build(args: BuildArgs) -> Result<()> {
    println!("pleme-linker build: Building node_modules");
    println!("  Manifest: {}", args.manifest.display());
    println!("  Output:   {}", args.output.display());
    println!();

    // Read manifest
    let manifest_content = fs::read_to_string(&args.manifest)
        .with_context(|| format!("Failed to read manifest {}", args.manifest.display()))?;
    let manifest: BuildManifest = serde_json::from_str(&manifest_content)
        .with_context(|| "Failed to parse manifest JSON")?;

    println!("Stage 1: Extracting {} packages...", manifest.packages.len());

    // Create output directory structure
    let node_modules = args.output.join("node_modules");
    let pnpm_store = node_modules.join(".pnpm");
    fs::create_dir_all(&pnpm_store)?;

    // Extract all packages to .pnpm store
    let mut extracted: HashMap<String, std::path::PathBuf> = HashMap::new();
    for pkg in &manifest.packages {
        let store_name = format!("{}@{}", pkg.pname.replace('/', "+"), pkg.version);
        let store_path = pnpm_store.join(&store_name).join("node_modules").join(&pkg.pname);

        // Handle scoped packages
        if pkg.pname.contains('/') {
            let scope = pkg.pname.split('/').next().unwrap();
            fs::create_dir_all(store_path.parent().unwrap().join(scope))?;
        }

        fs::create_dir_all(store_path.parent().unwrap())?;

        // Extract tarball
        extract_tarball(&pkg.tarball, &store_path)?;

        let key = format!("{}@{}", pkg.pname, pkg.version);
        extracted.insert(key, store_path);
    }

    println!("  Extracted {} packages", extracted.len());

    println!();
    println!("Stage 2: Creating node_modules symlinks...");

    // Create root node_modules symlinks following pnpm's approach:
    // 1. Root dependencies (from package.json) are hoisted first - these have priority
    // 2. For other packages, hoist the highest version (backward compatibility)
    // This ensures tools get the version explicitly requested by the developer
    let mut hoisted: HashMap<String, String> = HashMap::new();

    // First pass: hoist root dependencies with priority
    for root_dep in &manifest.root_dependencies {
        let (pname, _version) = parse_dep_spec(root_dep);
        if extracted.contains_key(root_dep) {
            hoisted.insert(pname, root_dep.clone());
        }
    }

    // Second pass: for packages not in root deps, hoist the highest version
    for pkg in &manifest.packages {
        let key = format!("{}@{}", pkg.pname, pkg.version);

        // Skip if already hoisted from root dependencies
        if hoisted.contains_key(&pkg.pname) {
            continue;
        }

        if let Some(existing_key) = hoisted.get(&pkg.pname) {
            // Compare versions - prefer higher version
            let existing_version = existing_key.rsplit('@').next().unwrap_or("0.0.0");
            if compare_versions(&pkg.version, existing_version) == std::cmp::Ordering::Greater {
                hoisted.insert(pkg.pname.clone(), key);
            }
        } else {
            hoisted.insert(pkg.pname.clone(), key);
        }
    }

    // Third pass: create symlinks for hoisted packages
    for (pname, key) in &hoisted {
        let store_path = extracted.get(key).unwrap();

        // Create symlink in root node_modules
        let link_path = if pname.contains('/') {
            let parts: Vec<&str> = pname.split('/').collect();
            let scope_dir = node_modules.join(parts[0]);
            fs::create_dir_all(&scope_dir)?;
            scope_dir.join(parts[1])
        } else {
            node_modules.join(pname)
        };

        // Create relative symlink
        let rel_path = diff_paths(store_path, link_path.parent().unwrap())
            .unwrap_or_else(|| store_path.clone());

        if link_path.symlink_metadata().is_ok() {
            fs::remove_file(&link_path).or_else(|_| fs::remove_dir_all(&link_path))?;
        }
        std::os::unix::fs::symlink(&rel_path, &link_path)?;
    }

    println!("  Created {} node_modules links", hoisted.len());
    if !manifest.root_dependencies.is_empty() {
        println!("  Root dependencies hoisted: {}", manifest.root_dependencies.len());
    }

    println!();
    println!("Stage 3: Creating nested dependency symlinks...");

    // Create nested node_modules for packages with specific version requirements
    let mut nested_count = 0;
    for pkg in &manifest.packages {
        for dep_spec in &pkg.dependencies {
            // Parse dep_spec like "@scope/name@version" or "name@version"
            let (dep_name, _dep_version) = parse_dep_spec(dep_spec);

            // Check if the hoisted version matches
            if let Some(hoisted_key) = hoisted.get(&dep_name) {
                if hoisted_key == dep_spec {
                    continue; // Hoisted version is correct
                }
            }

            // Need to create nested node_modules
            let pkg_key = format!("{}@{}", pkg.pname, pkg.version);
            let pkg_store_path = extracted.get(&pkg_key).unwrap();

            if let Some(dep_store_path) = extracted.get(dep_spec) {
                let nested_nm = pkg_store_path.join("node_modules");
                fs::create_dir_all(&nested_nm)?;

                let link_path = if dep_name.contains('/') {
                    let parts: Vec<&str> = dep_name.split('/').collect();
                    let scope_dir = nested_nm.join(parts[0]);
                    fs::create_dir_all(&scope_dir)?;
                    scope_dir.join(parts[1])
                } else {
                    nested_nm.join(&dep_name)
                };

                let rel_path = diff_paths(dep_store_path, link_path.parent().unwrap())
                    .unwrap_or_else(|| dep_store_path.clone());

                if link_path.symlink_metadata().is_ok() {
                    fs::remove_file(&link_path).or_else(|_| fs::remove_dir_all(&link_path))?;
                }
                std::os::unix::fs::symlink(&rel_path, &link_path)?;
                nested_count += 1;
            }
        }
    }

    println!("  Created {} nested dependency symlinks", nested_count);

    println!();
    println!("Stage 4: Linking workspace packages...");

    // Link workspace packages into node_modules as-is.
    //
    // Workspace packages ship TypeScript source — the consuming bundler (Vite)
    // compiles TS→JS at build time. No pre-built dist/ is required.
    // JavaScript artifacts are interim build products, not source of truth.
    for ws_pkg in &manifest.workspace_packages {
        let link_path = if ws_pkg.name.contains('/') {
            let parts: Vec<&str> = ws_pkg.name.split('/').collect();
            let scope_dir = node_modules.join(parts[0]);
            fs::create_dir_all(&scope_dir)?;
            scope_dir.join(parts[1])
        } else {
            node_modules.join(&ws_pkg.name)
        };

        if link_path.symlink_metadata().is_ok() {
            fs::remove_file(&link_path).or_else(|_| fs::remove_dir_all(&link_path))?;
        }
        std::os::unix::fs::symlink(&ws_pkg.path, &link_path)?;
        println!("  Linked {} -> {}", ws_pkg.name, ws_pkg.path.display());
    }

    println!();
    println!("Stage 5: Processing bin entries...");

    // Create .bin directory and symlinks
    let bin_dir = node_modules.join(".bin");
    fs::create_dir_all(&bin_dir)?;

    let mut bin_count = 0;
    for pkg in &manifest.packages {
        if !pkg.has_bin {
            continue;
        }

        let pkg_key = format!("{}@{}", pkg.pname, pkg.version);
        let pkg_path = extracted.get(&pkg_key).unwrap();
        let package_json_path = pkg_path.join("package.json");

        if let Ok(content) = fs::read_to_string(&package_json_path) {
            if let Ok(pkg_json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(bin) = pkg_json.get("bin") {
                    match bin {
                        serde_json::Value::String(path) => {
                            // Single binary with package name
                            let bin_name = pkg.pname.split('/').last().unwrap();
                            match create_bin_link(&bin_dir, bin_name, pkg_path, path, &args.node_bin) {
                                Ok(created) => { if created { bin_count += 1; } }
                                Err(e) => {
                                    eprintln!("  Warning: failed to create bin entry '{}' for {}: {}", bin_name, pkg_key, e);
                                }
                            }
                        }
                        serde_json::Value::Object(bins) => {
                            // Multiple binaries
                            for (name, path) in bins {
                                if let serde_json::Value::String(path_str) = path {
                                    match create_bin_link(&bin_dir, name, pkg_path, path_str, &args.node_bin) {
                                        Ok(created) => { if created { bin_count += 1; } }
                                        Err(e) => {
                                            eprintln!("  Warning: failed to create bin entry '{}' for {}: {}", name, pkg_key, e);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    println!("  Created {} bin entries", bin_count);

    println!();
    println!("Done!");
    println!("  Store packages: {}", extracted.len());
    println!("  Node modules links: {}", hoisted.len());
    println!("  Nested dependencies: {}", nested_count);
    println!("  Bin entries: {}", bin_count);

    Ok(())
}

/// Extract a tarball to a directory
///
/// Uses the Rust `tar` crate for extraction with full control over permissions.
/// npm tarballs often have directories with restrictive permissions (e.g., 666 without
/// execute bit) that would prevent accessing directory contents. We override all
/// permissions to be writable and executable for directories.
fn extract_tarball(tarball: &Path, dest: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use std::os::unix::fs::PermissionsExt;
    use tar::Archive;

    // Clean destination if it exists
    if dest.exists() {
        // Make writable first so we can remove
        make_writable_recursive(dest)?;
        fs::remove_dir_all(dest)?;
    }
    fs::create_dir_all(dest)?;

    // Open and decompress the tarball
    let file = fs::File::open(tarball)
        .with_context(|| format!("Failed to open tarball {}", tarball.display()))?;
    let gz = GzDecoder::new(file);
    let mut archive = Archive::new(gz);

    // Don't preserve permissions from the archive
    archive.set_preserve_permissions(false);

    // Extract each entry manually with proper permissions
    for entry_result in archive.entries()? {
        let mut entry = entry_result?;
        let entry_path = entry.path()?;

        // Strip the first component (usually "package/")
        let path_components: Vec<_> = entry_path.components().collect();
        if path_components.len() <= 1 {
            // Skip root entries or entries with only one component
            continue;
        }

        // Rebuild path without the first component
        let stripped_path: std::path::PathBuf =
            path_components[1..].iter().collect();
        let target_path = dest.join(&stripped_path);

        // Create parent directories if needed
        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
                // Ensure directory is accessible
                fs::set_permissions(parent, fs::Permissions::from_mode(0o755))?;
            }
        }

        // Handle entry type
        let entry_type = entry.header().entry_type();
        match entry_type {
            tar::EntryType::Directory => {
                fs::create_dir_all(&target_path)?;
                // Directories need execute bit to be accessible
                fs::set_permissions(&target_path, fs::Permissions::from_mode(0o755))?;
            }
            tar::EntryType::Regular | tar::EntryType::Continuous => {
                // Extract file
                entry.unpack(&target_path)?;
                // Determine if file should be executable:
                // 1. Check if it's in a bin/ directory
                // 2. Check if the archive permissions include execute bit
                // 3. Check if it starts with a shebang
                let archive_mode = entry.header().mode().unwrap_or(0o644);
                let should_be_executable = (archive_mode & 0o111) != 0
                    || stripped_path
                        .components()
                        .any(|c| c.as_os_str() == "bin");
                let mode = if should_be_executable { 0o755 } else { 0o644 };
                fs::set_permissions(&target_path, fs::Permissions::from_mode(mode))?;
            }
            tar::EntryType::Symlink | tar::EntryType::Link => {
                // Let tar crate handle symlinks
                entry.unpack_in(dest)?;
            }
            _ => {
                // For other types, try unpacking
                let _ = entry.unpack_in(dest);
            }
        }
    }

    Ok(())
}

/// Recursively make all files and directories writable
fn make_writable_recursive(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    if path.is_dir() {
        // First make the directory accessible and writable
        if let Ok(metadata) = fs::metadata(path) {
            let mut perms = metadata.permissions();
            perms.set_mode(perms.mode() | 0o700);
            let _ = fs::set_permissions(path, perms);
        }

        // Then process contents
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            make_writable_recursive(&entry.path())?;
        }
    }

    // Add write permission for owner
    if let Ok(metadata) = fs::metadata(path) {
        let mut perms = metadata.permissions();
        let mode = perms.mode();
        perms.set_mode(mode | 0o200);
        let _ = fs::set_permissions(path, perms);
    }

    Ok(())
}

/// Parse dependency spec like "@scope/name@version" or "name@version"
fn parse_dep_spec(spec: &str) -> (String, String) {
    if spec.starts_with('@') {
        // Scoped package: @scope/name@version
        if let Some(at_pos) = spec[1..].find('@') {
            let split_pos = at_pos + 1;
            (spec[..split_pos].to_string(), spec[split_pos + 1..].to_string())
        } else {
            (spec.to_string(), "*".to_string())
        }
    } else {
        // Regular package: name@version
        if let Some(at_pos) = spec.find('@') {
            (spec[..at_pos].to_string(), spec[at_pos + 1..].to_string())
        } else {
            (spec.to_string(), "*".to_string())
        }
    }
}

/// Create a bin symlink/wrapper
fn create_bin_link(
    bin_dir: &Path,
    name: &str,
    pkg_path: &Path,
    bin_path: &str,
    node_bin: &Path,
) -> Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    let link_path = bin_dir.join(name);
    let target = pkg_path.join(bin_path.trim_start_matches("./"));

    // Skip if the bin target doesn't exist in the extracted package
    if !target.exists() {
        eprintln!(
            "  Warning: bin target not found, skipping: {}",
            target.display()
        );
        return Ok(false);
    }

    // Create a shell wrapper that invokes node
    let wrapper = format!(
        "#!/bin/sh\nexec {} {} \"$@\"\n",
        node_bin.display(),
        target.display()
    );

    fs::write(&link_path, wrapper)?;
    fs::set_permissions(&link_path, fs::Permissions::from_mode(0o755))?;

    Ok(true)
}

/// Compare two semver versions
/// Returns Ordering::Greater if v1 > v2, Less if v1 < v2, Equal otherwise
fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<&str> = v.split('-').next().unwrap_or(v).split('.').collect();
        let major = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
        let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    };

    parse(v1).cmp(&parse(v2))
}
