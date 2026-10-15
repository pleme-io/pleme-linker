//! Shared types for pleme-linker

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

// ============================================================================
// NPM REGISTRY TYPES
// ============================================================================

/// Full package metadata from npm registry
#[derive(Debug, Clone, Deserialize)]
pub struct NpmPackageMetadata {
    #[allow(dead_code)]
    pub name: String,
    #[serde(rename = "dist-tags")]
    pub dist_tags: HashMap<String, String>,
    pub versions: HashMap<String, NpmVersionInfo>,
}

/// Version-specific info from npm registry
#[derive(Debug, Clone, Deserialize)]
pub struct NpmVersionInfo {
    #[allow(dead_code)]
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    #[serde(rename = "devDependencies", default)]
    #[allow(dead_code)]
    pub dev_dependencies: HashMap<String, String>,
    #[serde(rename = "peerDependencies", default)]
    pub peer_dependencies: HashMap<String, String>,
    #[serde(rename = "optionalDependencies", default)]
    pub optional_dependencies: HashMap<String, String>,
    pub dist: NpmDist,
    #[serde(default)]
    pub bin: Option<serde_json::Value>,
    #[serde(default)]
    pub os: Option<Vec<String>>,
    #[serde(default)]
    #[allow(dead_code)]
    pub cpu: Option<Vec<String>>,
}

/// Distribution info from npm registry
#[derive(Debug, Clone, Deserialize)]
pub struct NpmDist {
    pub tarball: String,
    pub integrity: Option<String>,
    pub shasum: Option<String>,
}

// ============================================================================
// RESOLUTION TYPES
// ============================================================================

/// Workspace package detected from file: dependencies
#[derive(Debug, Clone)]
pub struct WorkspacePackageRef {
    /// Package name (e.g., "@curupira/shared")
    pub name: String,
    /// Relative path from package.json (e.g., "../shared")
    pub relative_path: String,
}

/// A resolved package with all details needed for deps.nix
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedPackage {
    pub pname: String,
    pub version: String,
    pub url: String,
    pub integrity: String,
    pub dependencies: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_bin: Option<bool>,
}

/// Resolution context/state
pub struct ResolutionContext {
    /// HTTP client for registry requests
    pub client: Client,
    /// Registry URL
    pub registry: String,
    /// Target platform
    pub platform: String,
    /// Cache of fetched package metadata
    pub metadata_cache: HashMap<String, NpmPackageMetadata>,
    /// Resolved packages: "name@version" -> ResolvedPackage
    pub resolved: HashMap<String, ResolvedPackage>,
    /// Queue of packages to resolve: (name, version_constraint, is_optional)
    pub queue: VecDeque<(String, String, bool)>,
    /// Root dependencies (direct dependencies from package.json)
    pub root_deps: Vec<String>,
    /// Packages that failed to resolve (for reporting)
    pub failed: Vec<(String, String, String)>, // (name, constraint, error)
    /// Already queued packages (to avoid duplicates)
    pub queued: HashSet<String>,
    /// Workspace packages detected from file: dependencies
    pub workspace_packages: Vec<WorkspacePackageRef>,
}

impl ResolutionContext {
    pub fn new(registry: String, platform: String) -> Self {
        Self {
            client: Client::new(),
            registry,
            platform,
            metadata_cache: HashMap::new(),
            resolved: HashMap::new(),
            queue: VecDeque::new(),
            root_deps: Vec::new(),
            failed: Vec::new(),
            queued: HashSet::new(),
            workspace_packages: Vec::new(),
        }
    }
}

/// npm alias info: "npm:package@version"
#[derive(Debug)]
pub struct NpmAlias {
    pub target_package: String,
    pub target_constraint: String,
}

// ============================================================================
// BUILD TYPES
// ============================================================================

/// Manifest entry for build command
#[derive(Debug, Deserialize)]
pub struct BuildManifestEntry {
    pub pname: String,
    pub version: String,
    pub tarball: PathBuf,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(rename = "hasBin", default)]
    pub has_bin: bool,
}

/// Workspace package from manifest
#[derive(Debug, Deserialize)]
pub struct WorkspacePackageEntry {
    pub name: String,
    pub path: PathBuf,
}

/// Full build manifest
#[derive(Debug, Deserialize)]
pub struct BuildManifest {
    pub packages: Vec<BuildManifestEntry>,
    #[serde(rename = "workspacePackages", default)]
    pub workspace_packages: Vec<WorkspacePackageEntry>,
    /// Root dependencies from package.json (e.g., ["typescript@5.9.3", "react@18.3.1"])
    /// These are hoisted to root node_modules with priority over transitive deps
    /// Follows pnpm's approach: only direct dependencies are accessible at root
    #[serde(rename = "rootDependencies", default)]
    pub root_dependencies: Vec<String>,
}
