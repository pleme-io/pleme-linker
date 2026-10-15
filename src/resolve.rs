//! npm registry resolution and deps.nix generation

use anyhow::{Context, Result};
use chrono::Utc;
use futures::future::join_all;
use semver::{Version, VersionReq};
use std::collections::HashSet;
use std::fs;

use crate::cli::ResolveArgs;
use crate::types::{
    NpmAlias, NpmPackageMetadata, NpmVersionInfo, ResolutionContext, ResolvedPackage,
    WorkspacePackageRef,
};
use crate::utils::escape_nix_string;

/// Maximum number of concurrent metadata fetches
const MAX_CONCURRENT_FETCHES: usize = 32;

/// Run the resolve command
pub async fn run_resolve(args: ResolveArgs) -> Result<()> {
    println!("pleme-linker resolve: Resolving dependencies");
    println!("  Project:  {}", args.project.display());
    println!("  Registry: {}", args.registry);
    println!("  Platform: {}", args.platform);
    println!();

    // Read package.json
    let package_json_path = args.project.join("package.json");
    let package_json_content = fs::read_to_string(&package_json_path)
        .with_context(|| format!("Failed to read {}", package_json_path.display()))?;
    let package_json: serde_json::Value = serde_json::from_str(&package_json_content)
        .with_context(|| "Failed to parse package.json")?;

    // Initialize resolution context
    let mut ctx = ResolutionContext::new(args.registry.clone(), args.platform.clone());

    // Queue root dependencies
    let mut dep_count = 0;
    if let Some(deps) = package_json.get("dependencies").and_then(|d| d.as_object()) {
        for (name, version) in deps {
            let version_str = version.as_str().unwrap_or("*");

            // Collect workspace/file dependencies (local packages)
            if version_str.starts_with("workspace:")
                || version_str.starts_with("file:")
                || version_str.starts_with("link:")
            {
                let relative_path = if version_str.starts_with("file:") {
                    version_str.strip_prefix("file:").unwrap_or(version_str).to_string()
                } else if version_str.starts_with("link:") {
                    version_str.strip_prefix("link:").unwrap_or(version_str).to_string()
                } else {
                    "*".to_string()
                };

                ctx.workspace_packages.push(WorkspacePackageRef {
                    name: name.clone(),
                    relative_path,
                });
                println!("  Found workspace dependency: {} -> {}", name, version_str);
                continue;
            }

            ctx.queue.push_back((name.clone(), version_str.to_string(), false));
            ctx.queued.insert(format!("{}@{}", name, version_str));
            dep_count += 1;
        }
    }

    if args.include_dev {
        if let Some(deps) = package_json
            .get("devDependencies")
            .and_then(|d| d.as_object())
        {
            for (name, version) in deps {
                let version_str = version.as_str().unwrap_or("*");

                // Skip workspace/file dependencies
                if version_str.starts_with("workspace:")
                    || version_str.starts_with("file:")
                    || version_str.starts_with("link:")
                {
                    continue;
                }

                let key = format!("{}@{}", name, version_str);
                if !ctx.queued.contains(&key) {
                    ctx.queue.push_back((name.clone(), version_str.to_string(), false));
                    ctx.queued.insert(key);
                    dep_count += 1;
                }
            }
        }
    }

    println!("  Found {} direct dependencies", dep_count);
    println!();
    println!("Resolving dependency tree (parallel fetching, {} concurrent)...", MAX_CONCURRENT_FETCHES);

    // Parallel resolution loop
    let mut resolved_count = 0;

    while !ctx.queue.is_empty() {
        // Collect batch of packages to fetch metadata for
        let mut batch: Vec<(String, String, bool, String)> = Vec::new(); // (name, constraint, is_optional, target_package)
        let mut packages_to_fetch: HashSet<String> = HashSet::new();

        // Take up to MAX_CONCURRENT_FETCHES items from queue
        while let Some((name, constraint, is_optional)) = ctx.queue.pop_front() {
            let (actual_name, actual_constraint, target_package) =
                if let Some(alias) = parse_npm_alias(&constraint) {
                    (name.clone(), alias.target_constraint.clone(), alias.target_package.clone())
                } else {
                    (name.clone(), constraint.clone(), name.clone())
                };

            // Skip if already satisfied
            if is_satisfied(&ctx, &actual_name, &actual_constraint) {
                continue;
            }

            // Skip if we already have metadata cached
            if !ctx.metadata_cache.contains_key(&target_package) {
                packages_to_fetch.insert(target_package.clone());
            }

            batch.push((name, constraint, is_optional, target_package));

            if packages_to_fetch.len() >= MAX_CONCURRENT_FETCHES {
                break;
            }
        }

        if batch.is_empty() {
            continue;
        }

        // Fetch all metadata in parallel
        if !packages_to_fetch.is_empty() {
            let fetch_results = fetch_metadata_batch(&ctx.client, &ctx.registry, &packages_to_fetch).await;

            // Store fetched metadata in cache
            for (name, result) in fetch_results {
                match result {
                    Ok(metadata) => {
                        ctx.metadata_cache.insert(name, metadata);
                    }
                    Err(e) => {
                        // Mark all packages depending on this as failed
                        for (batch_name, batch_constraint, is_optional, target) in &batch {
                            if target == &name && !*is_optional {
                                ctx.failed.push((batch_name.clone(), batch_constraint.clone(), e.clone()));
                            }
                        }
                    }
                }
            }
        }

        // Process batch with cached metadata
        for (name, constraint, is_optional, target_package) in batch {
            let (actual_name, actual_constraint, _) =
                if let Some(alias) = parse_npm_alias(&constraint) {
                    (name.clone(), alias.target_constraint.clone(), alias.target_package.clone())
                } else {
                    (name.clone(), constraint.clone(), name.clone())
                };

            let metadata = match ctx.metadata_cache.get(&target_package) {
                Some(m) => m.clone(),
                None => {
                    if !is_optional {
                        // Already logged in fetch phase
                    }
                    continue;
                }
            };

            let version_info = match resolve_version(&metadata, &actual_constraint) {
                Some(v) => v.clone(),
                None => {
                    if !is_optional {
                        ctx.failed.push((
                            name.clone(),
                            constraint.clone(),
                            format!("No version satisfies constraint {}", actual_constraint),
                        ));
                    }
                    continue;
                }
            };

            // Skip platform-incompatible packages
            if !is_platform_compatible(&version_info, &ctx.platform) {
                continue;
            }

            let key = format!("{}@{}", actual_name, version_info.version);
            if ctx.resolved.contains_key(&key) {
                continue;
            }

            // Get integrity hash
            let integrity = version_info
                .dist
                .integrity
                .clone()
                .or_else(|| version_info.dist.shasum.as_ref().map(|s| format!("sha1-{}", s)))
                .unwrap_or_default();

            // Collect all dependencies
            let mut dep_keys = Vec::new();
            for (dep_name, dep_constraint) in &version_info.dependencies {
                let dep_key = queue_dependency(&mut ctx, dep_name, dep_constraint, false);
                if let Some(k) = dep_key {
                    dep_keys.push(k);
                }
            }

            for (dep_name, dep_constraint) in &version_info.optional_dependencies {
                // Queue optional dependencies for resolution
                // Also track them in dep_keys so build.rs can create proper nested
                // symlinks when versions don't match (critical for esbuild platform binaries)
                let dep_key = queue_dependency(&mut ctx, dep_name, dep_constraint, true);
                if let Some(k) = dep_key {
                    dep_keys.push(k);
                }
            }

            for (dep_name, dep_constraint) in &version_info.peer_dependencies {
                queue_dependency(&mut ctx, dep_name, dep_constraint, true);
            }

            // Add to root deps if it's a direct dependency
            let root_key = format!("{}@{}", actual_name, version_info.version);
            if dep_count > 0 && ctx.root_deps.len() < dep_count * 2 {
                ctx.root_deps.push(root_key.clone());
            }

            // Store resolved package
            ctx.resolved.insert(
                key,
                ResolvedPackage {
                    pname: actual_name.clone(),
                    version: version_info.version.clone(),
                    url: version_info.dist.tarball.clone(),
                    integrity,
                    dependencies: dep_keys,
                    has_bin: version_info.bin.as_ref().map(|_| true),
                },
            );

            resolved_count += 1;
        }

        print!("\r  Resolved {} packages...", resolved_count);
        use std::io::Write;
        std::io::stdout().flush().ok();
    }

    println!("\r  Resolved {} packages total     ", ctx.resolved.len());

    // Post-process: Update dependency keys from constraints to resolved versions
    // Before: dependencies = ["jsonfile@^6.0.1"]
    // After:  dependencies = ["jsonfile@6.2.0"]
    let resolved_clone: Vec<(String, ResolvedPackage)> = ctx.resolved.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    for (_key, pkg) in &mut ctx.resolved {
        let mut updated_deps = Vec::new();
        for dep_constraint_key in &pkg.dependencies {
            // Parse the constraint key to get name and constraint
            let (dep_name, dep_constraint) = if dep_constraint_key.starts_with('@') {
                // Scoped package: @scope/name@constraint
                if let Some(at_pos) = dep_constraint_key[1..].find('@') {
                    let split_pos = at_pos + 1;
                    (
                        dep_constraint_key[..split_pos].to_string(),
                        dep_constraint_key[split_pos + 1..].to_string(),
                    )
                } else {
                    (dep_constraint_key.clone(), "*".to_string())
                }
            } else {
                // Regular package: name@constraint
                if let Some(at_pos) = dep_constraint_key.find('@') {
                    (
                        dep_constraint_key[..at_pos].to_string(),
                        dep_constraint_key[at_pos + 1..].to_string(),
                    )
                } else {
                    (dep_constraint_key.clone(), "*".to_string())
                }
            };

            // Find the resolved version for this dependency
            // Look through all resolved packages to find one that matches the name and constraint
            let mut found_version = None;
            for (_, resolved_pkg) in &resolved_clone {
                if resolved_pkg.pname == dep_name {
                    // Check if this version satisfies the constraint
                    if let Some(req) = parse_version_req(&dep_constraint) {
                        if let Ok(ver) = Version::parse(&resolved_pkg.version) {
                            if req.matches(&ver) {
                                found_version = Some(format!("{}@{}", dep_name, resolved_pkg.version));
                                break;
                            }
                        }
                    }
                }
            }

            if let Some(resolved_key) = found_version {
                updated_deps.push(resolved_key);
            } else {
                // Keep original if no match found (shouldn't happen normally)
                updated_deps.push(dep_constraint_key.clone());
            }
        }
        pkg.dependencies = updated_deps;
    }

    // Report failures
    if !ctx.failed.is_empty() {
        println!();
        println!("Warning: {} packages failed to resolve:", ctx.failed.len());
        for (name, constraint, error) in &ctx.failed {
            println!("  {} @ {}: {}", name, constraint, error);
        }
    }

    // Generate deps.nix
    let output_path = args.output.unwrap_or_else(|| args.project.join("deps.nix"));
    println!();
    println!("Generating {}...", output_path.display());

    let nix_content = generate_deps_nix(&ctx);
    fs::write(&output_path, nix_content)?;

    println!();
    println!("Done!");
    println!("  Output: {}", output_path.display());
    println!("  Packages: {}", ctx.resolved.len());
    println!("  Root dependencies: {}", ctx.root_deps.len());

    Ok(())
}

/// Fetch multiple package metadata from registry in parallel
async fn fetch_metadata_batch(
    client: &reqwest::Client,
    registry: &str,
    packages: &HashSet<String>,
) -> Vec<(String, Result<NpmPackageMetadata, String>)> {
    let futures: Vec<_> = packages
        .iter()
        .map(|name| {
            let client = client.clone();
            let registry = registry.to_string();
            let name = name.clone();
            async move {
                let result = fetch_single_metadata(&client, &registry, &name).await;
                (name, result)
            }
        })
        .collect();

    join_all(futures).await
}

/// Fetch single package metadata from registry
async fn fetch_single_metadata(
    client: &reqwest::Client,
    registry: &str,
    name: &str,
) -> Result<NpmPackageMetadata, String> {
    let url = if name.starts_with('@') {
        let encoded = name.replace('/', "%2f");
        format!("{}/{}", registry, encoded)
    } else {
        format!("{}/{}", registry, name)
    };

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch metadata for {}: {}", name, e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to fetch {}: HTTP {}",
            name,
            response.status()
        ));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse metadata for {}: {}", name, e))
}

/// Check if a version constraint is already satisfied
fn is_satisfied(ctx: &ResolutionContext, name: &str, constraint: &str) -> bool {
    let req = match parse_version_req(constraint) {
        Some(r) => r,
        None => return false,
    };

    ctx.resolved.iter().any(|(_key, pkg)| {
        if pkg.pname != name {
            return false;
        }
        if let Ok(version) = Version::parse(&pkg.version) {
            req.matches(&version)
        } else {
            false
        }
    })
}

/// Resolve best matching version
fn resolve_version<'a>(metadata: &'a NpmPackageMetadata, constraint: &str) -> Option<&'a NpmVersionInfo> {
    // Handle "latest" tag
    if constraint == "latest" || constraint == "*" {
        if let Some(latest_version) = metadata.dist_tags.get("latest") {
            return metadata.versions.get(latest_version);
        }
    }

    // Handle explicit tags
    if let Some(version) = metadata.dist_tags.get(constraint) {
        return metadata.versions.get(version);
    }

    // Parse as semver constraint
    let req = parse_version_req(constraint)?;

    // Find best matching version
    let mut best: Option<(&String, &NpmVersionInfo)> = None;
    for (version_str, info) in &metadata.versions {
        if let Ok(version) = Version::parse(version_str) {
            if req.matches(&version) {
                match &best {
                    None => best = Some((version_str, info)),
                    Some((best_str, _)) => {
                        if let Ok(best_ver) = Version::parse(best_str) {
                            if version > best_ver {
                                best = Some((version_str, info));
                            }
                        }
                    }
                }
            }
        }
    }

    best.map(|(_, info)| info)
}

/// Parse version requirement, handling npm-specific formats
fn parse_version_req(constraint: &str) -> Option<VersionReq> {
    let cleaned = constraint
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('=');

    // Handle x-ranges
    let cleaned = cleaned
        .replace(".x", "")
        .replace(".X", "")
        .replace(".*", "");

    // Handle || (or) - take first option
    let cleaned = if let Some(pos) = cleaned.find("||") {
        cleaned[..pos].trim().to_string()
    } else {
        cleaned.to_string()
    };

    // Handle npm hyphen ranges: "1 - 2" -> ">=1, <=2"
    // Must check before space-separated handling
    if let Some(result) = parse_hyphen_range(&cleaned) {
        return Some(result);
    }

    // Handle space-separated ranges: ">=1.0.0 <2.0.0" -> ">=1.0.0, <2.0.0"
    // npm uses spaces to AND constraints together
    if let Some(result) = parse_space_separated_range(&cleaned) {
        return Some(result);
    }

    VersionReq::parse(&cleaned).ok()
}

/// Parse npm hyphen range: "1.2.3 - 2.3.4" -> ">=1.2.3, <=2.3.4"
fn parse_hyphen_range(constraint: &str) -> Option<VersionReq> {
    // Look for " - " pattern (space-hyphen-space)
    let parts: Vec<&str> = constraint.split(" - ").collect();
    if parts.len() == 2 {
        let lower = parts[0].trim();
        let upper = parts[1].trim();

        // Validate both parts look like versions (start with digit)
        if !lower.is_empty() && !upper.is_empty()
           && lower.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
           && upper.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
        {
            // Convert to semver range: >=lower, <=upper
            let semver_range = format!(">={}, <={}", lower, upper);
            return VersionReq::parse(&semver_range).ok();
        }
    }
    None
}

/// Parse space-separated constraints: ">=1.0.0 <2.0.0" -> ">=1.0.0, <2.0.0"
fn parse_space_separated_range(constraint: &str) -> Option<VersionReq> {
    // Split on spaces, but keep operators attached to versions
    let parts: Vec<&str> = constraint.split_whitespace().collect();

    if parts.len() < 2 {
        return None;
    }

    // Check if this looks like space-separated constraints
    // Each part should start with an operator or be a version
    let mut constraints: Vec<String> = Vec::new();
    let mut i = 0;

    while i < parts.len() {
        let part = parts[i];

        // If part starts with operator, it's a complete constraint
        if part.starts_with(">=") || part.starts_with("<=") ||
           part.starts_with('>') || part.starts_with('<') ||
           part.starts_with('^') || part.starts_with('~') {
            constraints.push(part.to_string());
            i += 1;
        }
        // If part is just an operator, combine with next part
        else if (part == ">=" || part == "<=" || part == ">" || part == "<") && i + 1 < parts.len() {
            constraints.push(format!("{}{}", part, parts[i + 1]));
            i += 2;
        }
        // Plain version number - treat as exact match
        else if part.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            constraints.push(format!("={}", part));
            i += 1;
        }
        else {
            // Unknown format, bail out
            return None;
        }
    }

    if constraints.len() >= 2 {
        let semver_range = constraints.join(", ");
        return VersionReq::parse(&semver_range).ok();
    }

    None
}

/// Parse npm alias: "npm:package@version"
fn parse_npm_alias(constraint: &str) -> Option<NpmAlias> {
    if !constraint.starts_with("npm:") {
        return None;
    }

    let rest = &constraint[4..];
    let (package, version) = if let Some(at_pos) = rest.rfind('@') {
        if at_pos == 0 {
            // Scoped package like npm:@scope/pkg
            if let Some(second_at) = rest[1..].find('@') {
                let pos = second_at + 1;
                (&rest[..pos], &rest[pos + 1..])
            } else {
                (rest, "*")
            }
        } else {
            (&rest[..at_pos], &rest[at_pos + 1..])
        }
    } else {
        (rest, "*")
    };

    Some(NpmAlias {
        target_package: package.to_string(),
        target_constraint: version.to_string(),
    })
}

/// Check platform compatibility
fn is_platform_compatible(info: &NpmVersionInfo, platform: &str) -> bool {
    if let Some(os_list) = &info.os {
        let blocked = format!("!{}", platform);
        if os_list.iter().any(|os| os == &blocked) {
            return false;
        }
        if !os_list.is_empty()
            && !os_list.iter().any(|os| os == platform || os.starts_with('!'))
        {
            return false;
        }
    }
    true
}

/// Queue a dependency for resolution
fn queue_dependency(
    ctx: &mut ResolutionContext,
    name: &str,
    constraint: &str,
    is_optional: bool,
) -> Option<String> {
    // Skip workspace/file dependencies
    if constraint.starts_with("workspace:")
        || constraint.starts_with("file:")
        || constraint.starts_with("link:")
    {
        return None;
    }

    let key = format!("{}@{}", name, constraint);
    if !ctx.queued.contains(&key) {
        ctx.queue.push_back((name.to_string(), constraint.to_string(), is_optional));
        ctx.queued.insert(key.clone());
    }
    Some(key)
}

/// Generate deps.nix content
fn generate_deps_nix(ctx: &ResolutionContext) -> String {
    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");

    // Sort packages for deterministic output
    let mut packages: Vec<_> = ctx.resolved.iter().collect();
    packages.sort_by(|a, b| a.0.cmp(b.0));

    // Sort root deps
    let mut root_deps = ctx.root_deps.clone();
    root_deps.sort();

    // Sort workspace packages
    let mut workspace_packages = ctx.workspace_packages.clone();
    workspace_packages.sort_by(|a, b| a.name.cmp(&b.name));

    // Generate package entries
    let package_entries: Vec<String> = packages
        .iter()
        .map(|(key, pkg)| {
            let deps_str = if pkg.dependencies.is_empty() {
                String::new()
            } else {
                let deps: Vec<String> = pkg.dependencies.iter().map(|d| format!("\"{}\"", d)).collect();
                format!("\n      dependencies = [ {} ];", deps.join(" "))
            };

            let bin_str = if pkg.has_bin == Some(true) {
                "\n      hasBin = true;"
            } else {
                ""
            };

            format!(
                r#"    "{}" = {{
      pname = "{}";
      version = "{}";
      url = "{}";
      integrity = "{}";{}{}
    }};"#,
                key,
                escape_nix_string(&pkg.pname),
                pkg.version,
                escape_nix_string(&pkg.url),
                escape_nix_string(&pkg.integrity),
                deps_str,
                bin_str
            )
        })
        .collect();

    let root_deps_str: Vec<String> = root_deps.iter().map(|d| format!("    \"{}\"", d)).collect();

    // Generate workspace packages section
    let workspace_section = if workspace_packages.is_empty() {
        String::new()
    } else {
        let workspace_entries: Vec<String> = workspace_packages
            .iter()
            .map(|wp| {
                format!(
                    r#"    {{ name = "{}"; path = "{}"; }}"#,
                    escape_nix_string(&wp.name),
                    escape_nix_string(&wp.relative_path)
                )
            })
            .collect();

        format!(
            r#"

  # Workspace packages (local file: dependencies)
  # These are built from source by pleme-linker build-project
  workspacePackages = [
{}
  ];"#,
            workspace_entries.join("\n")
        )
    };

    format!(
        r#"# Generated by pleme-linker resolve
# DO NOT EDIT - regenerate with: pleme-linker resolve --project .
#
# This file IS the lockfile. It contains:
# - All resolved packages with exact versions
# - Tarball URLs and integrity hashes (from npm registry)
# - Dependency relationships
# - Workspace packages (local file: dependencies)
#
# Nix uses fetchurl to download each package (cached in Attic),
# then pleme-linker build assembles node_modules.
{{
  # Metadata
  generatedAt = "{timestamp}";
  resolverVersion = "0.3.0";
  packageCount = {package_count};

  # Root dependencies (direct deps from package.json)
  rootDependencies = [
{root_deps}
  ];

  # All resolved packages
  packages = {{
{packages}
  }};{workspace_section}
}}
"#,
        timestamp = timestamp,
        package_count = packages.len(),
        root_deps = root_deps_str.join("\n"),
        packages = package_entries.join("\n\n"),
        workspace_section = workspace_section
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hyphen_range() {
        // "1 - 2" should become ">=1, <=2"
        let result = parse_hyphen_range("1 - 2");
        assert!(result.is_some(), "Should parse hyphen range '1 - 2'");

        // "1.2.3 - 2.3.4" should become ">=1.2.3, <=2.3.4"
        let result = parse_hyphen_range("1.2.3 - 2.3.4");
        assert!(result.is_some(), "Should parse hyphen range '1.2.3 - 2.3.4'");

        // Not a hyphen range
        let result = parse_hyphen_range(">=1.0.0");
        assert!(result.is_none(), "Should not parse non-hyphen range");
    }

    #[test]
    fn test_parse_space_separated_range() {
        // ">=1.0.0 <2.0.0" should become ">=1.0.0, <2.0.0"
        let result = parse_space_separated_range(">=1.0.0 <2.0.0");
        assert!(result.is_some(), "Should parse space-separated range '>=1.0.0 <2.0.0'");

        // ">=3.1.1 <6" - real example from npm packages
        let result = parse_space_separated_range(">=3.1.1 <6");
        assert!(result.is_some(), "Should parse space-separated range '>=3.1.1 <6'");

        // ">=0.5 0" - another npm format
        let result = parse_space_separated_range(">=0.5 <1");
        assert!(result.is_some(), "Should parse space-separated range '>=0.5 <1'");

        // Single constraint should not be parsed
        let result = parse_space_separated_range(">=1.0.0");
        assert!(result.is_none(), "Should not parse single constraint");
    }

    #[test]
    fn test_parse_version_req_npm_formats() {
        // Test that parse_version_req handles the problematic npm formats

        // Hyphen range
        let result = parse_version_req("1 - 2");
        assert!(result.is_some(), "parse_version_req should handle '1 - 2'");

        // Space-separated constraints
        let result = parse_version_req(">=3.1.1 <6");
        assert!(result.is_some(), "parse_version_req should handle '>=3.1.1 <6'");

        // Standard semver (should still work)
        let result = parse_version_req("^1.0.0");
        assert!(result.is_some(), "parse_version_req should handle '^1.0.0'");

        let result = parse_version_req(">=1.0.0");
        assert!(result.is_some(), "parse_version_req should handle '>=1.0.0'");

        let result = parse_version_req("~1.0.0");
        assert!(result.is_some(), "parse_version_req should handle '~1.0.0'");
    }
}
