//! Shared utility functions

use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

/// Compute relative path from `base` to `target`.
/// Returns the path you'd need to write in `base` to reach `target`.
pub fn diff_paths(target: &Path, base: &Path) -> Option<PathBuf> {
    let target = target
        .canonicalize()
        .ok()
        .unwrap_or_else(|| target.to_path_buf());
    let base = base
        .canonicalize()
        .ok()
        .unwrap_or_else(|| base.to_path_buf());

    let mut target_components = target.components().peekable();
    let mut base_components = base.components().peekable();

    // Skip common prefix
    while target_components.peek() == base_components.peek() {
        if target_components.peek().is_none() {
            break;
        }
        target_components.next();
        base_components.next();
    }

    // For each remaining component in base, add ".."
    let mut result = PathBuf::new();
    for component in base_components {
        if matches!(component, Component::Normal(_)) {
            result.push("..");
        }
    }

    // Add remaining target components
    for component in target_components {
        result.push(component);
    }

    if result.as_os_str().is_empty() {
        Some(PathBuf::from("."))
    } else {
        Some(result)
    }
}

/// Recursively copy a directory
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if matches!(
                name_str.as_ref(),
                "node_modules" | "dist" | ".git" | ".turbo"
            ) {
                continue;
            }
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path)?;
            let mut perms = fs::metadata(&dst_path)?.permissions();
            perms.set_mode(perms.mode() | 0o644);
            fs::set_permissions(&dst_path, perms)?;
        } else if file_type.is_symlink() {
            let target = fs::read_link(&src_path)?;
            let _ = fs::remove_file(&dst_path);
            std::os::unix::fs::symlink(&target, &dst_path)?;
        }
    }

    Ok(())
}

/// Copy workspace package recursively, excluding build artifacts
#[allow(dead_code)]
pub fn copy_workspace_package_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .with_context(|| format!("Failed to create directory: {}", dst.display()))?;

    let entries = fs::read_dir(src)
        .with_context(|| format!("Failed to read directory: {}", src.display()))?;

    for entry in entries {
        let entry =
            entry.with_context(|| format!("Failed to read entry in: {}", src.display()))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip build artifacts, hidden files, and node_modules
        if matches!(
            name_str.as_ref(),
            "node_modules" | "dist" | ".git" | ".turbo" | "coverage"
        ) || name_str.starts_with('.')
        {
            continue;
        }

        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to get file type: {}", src_path.display()))?;

        if file_type.is_dir() {
            copy_workspace_package_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "Failed to copy {} -> {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;

            let mut perms = fs::metadata(&dst_path)?.permissions();
            perms.set_mode(perms.mode() | 0o644);
            fs::set_permissions(&dst_path, perms)?;
        }
        // Skip symlinks in source
    }

    Ok(())
}

/// Modify package.json content to point to source files instead of dist
#[allow(dead_code)]
pub fn modify_package_json_content(content: &str) -> Result<String> {
    let mut pkg: serde_json::Value = serde_json::from_str(content)?;

    fn dist_to_src(path: &str) -> String {
        path.replace("./dist/", "./src/")
            .replace(".js", ".ts")
            .replace(".d.ts", ".ts")
    }

    if let Some(main) = pkg.get("main").and_then(|v| v.as_str()) {
        pkg["main"] = serde_json::Value::String(dist_to_src(main));
    }

    if let Some(module) = pkg.get("module").and_then(|v| v.as_str()) {
        pkg["module"] = serde_json::Value::String(dist_to_src(module));
    }

    if let Some(types) = pkg.get("types").and_then(|v| v.as_str()) {
        pkg["types"] = serde_json::Value::String(dist_to_src(types));
    }

    if let Some(exports) = pkg.get_mut("exports") {
        update_exports_for_source(exports);
    }

    Ok(serde_json::to_string_pretty(&pkg)?)
}

/// Recursively update exports object to point to source
#[allow(dead_code)]
fn update_exports_for_source(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => {
            if s.contains("./dist/") {
                *s = s
                    .replace("./dist/", "./src/")
                    .replace(".js", ".ts")
                    .replace(".d.ts", ".ts");
            }
        }
        serde_json::Value::Object(obj) => {
            for (_, v) in obj.iter_mut() {
                update_exports_for_source(v);
            }
        }
        _ => {}
    }
}

/// Compare semver versions
#[allow(dead_code)]
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    use semver::Version;
    match (Version::parse(a), Version::parse(b)) {
        (Ok(va), Ok(vb)) => va.cmp(&vb),
        _ => a.cmp(b),
    }
}

/// Escape a string for Nix
pub fn escape_nix_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
}
