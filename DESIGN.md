# pleme-linker: Nix-Native JavaScript Package Manager

A Rust-based package manager designed from the ground up for Nix builds. Instead of fighting npm/pnpm/yarn in the Nix sandbox, we own the entire pipeline.

## Vision

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           pleme-linker                                      │
│                   Nix-Native JavaScript Package Manager                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  COMMANDS:                                                                  │
│  ─────────                                                                  │
│  resolve        package.json → deps.nix (query registry, resolve versions) │
│  build          deps.nix + tarballs → node_modules (pure, no network)      │
│  build-project  Full TypeScript project (node_modules + OXC/Vite + wrap)   │
│  build-library  TypeScript library via tsdown (produces dist/)             │
│  regen          Regenerate deps.nix for web projects                       │
│                                                                             │
│  KEY INSIGHT:                                                               │
│  ────────────                                                               │
│  The npm registry is just HTTP. We don't need npm/pnpm/yarn at all.        │
│  Output IS Nix expressions, not a lockfile that needs parsing.             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Why?

| Approach | Complexity | Ownership | Nix Integration |
|----------|------------|-----------|-----------------|
| dream2nix | High | None | Fighting it |
| pnpm + parsing | Medium | Partial | Bridging |
| **pleme-linker** | Low | 100% | Native |

Current tools expect network access during install, have complex lockfile formats, and fight with Nix's sandbox model. We're bridging two incompatible paradigms.

pleme-linker is designed for Nix from day one:
- **Nix-native output**: Generated files ARE Nix expressions
- **Per-package caching**: Each package fetched separately (works with Attic/Cachix)
- **No network in sandbox**: Fetching done by Nix's fetchurl, building is pure
- **100% ownership**: We control resolution, not npm/pnpm

---

## Implementation Phases

### Phase 1: Single Project (MVP) ✓
- [x] `build` command - build node_modules from fetched tarballs (pnpm-style store)
- [x] `resolve` command - query npm registry, generate deps.nix (parallel fetching)
- [x] `build-project` command - full TypeScript project builds (OXC or Vite)
- [x] `build-library` command - TypeScript library builds via tsdown
- [x] `regen` command - regenerate deps.nix for web projects
- [x] Workspace package support (`workspace:`, `file:`, `link:` deps in package.json)
- [x] npm alias support (`npm:package@version`)
- [x] Platform filtering (linux/darwin)
- [x] OXC-powered TypeScript compilation (pure Rust, no Node.js for compilation)

### Phase 2: Workspace Support
- [ ] `pleme.toml` configuration format
- [ ] `init` command - create workspace config
- [ ] Workspace package discovery (glob patterns)
- [ ] Topological sort for build order
- [ ] Per-package deps.nix generation

### Phase 3: Polish
- [ ] `add` command - add dependencies
- [ ] `info` command - show workspace info
- [ ] Better error messages
- [ ] Registry response caching

### Phase 4: Release
- [ ] Documentation
- [ ] Examples
- [ ] CI/CD
- [ ] Publish as standalone tool

---

## Architecture

### Resolution Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         pleme-linker resolve                                │
│                                                                             │
│  Input: package.json                                                        │
│  Output: deps.nix (Nix attrset, IS the lockfile)                           │
│                                                                             │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐         │
│  │  package.json   │───▶│  npm Registry   │───▶│    deps.nix     │         │
│  │  Parser         │    │  Client         │    │    Generator    │         │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘         │
│          │                      │                      │                    │
│          ▼                      ▼                      ▼                    │
│  ┌─────────────────────────────────────────────────────────────────┐       │
│  │                    Resolution Engine                             │       │
│  │                                                                  │       │
│  │  1. Parse root dependencies from package.json                   │       │
│  │  2. Queue: [(name, version_constraint)]                         │       │
│  │  3. For each queued package:                                    │       │
│  │     a. Fetch metadata from registry (cached)                    │       │
│  │     b. Resolve version constraint → specific version            │       │
│  │     c. Extract: tarball URL, integrity, dependencies            │       │
│  │     d. Queue transitive dependencies                            │       │
│  │  4. Handle version conflicts (highest compatible wins)          │       │
│  │  5. Output resolved tree as Nix expression                      │       │
│  └─────────────────────────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────────────────────┘
```

### npm Registry API

```
GET https://registry.npmjs.org/{package}
GET https://registry.npmjs.org/@scope%2Fname  (URL-encoded for scoped)

Response:
{
  "name": "react",
  "dist-tags": { "latest": "18.2.0" },
  "versions": {
    "18.2.0": {
      "dependencies": { "loose-envify": "^1.1.0" },
      "peerDependencies": { "react": "^18.0.0" },
      "optionalDependencies": { "fsevents": "^2.3.0" },
      "os": ["darwin"],
      "cpu": ["x64"],
      "dist": {
        "tarball": "https://registry.npmjs.org/react/-/react-18.2.0.tgz",
        "integrity": "sha512-...",
        "shasum": "..."
      }
    }
  }
}
```

### Output Format (deps.nix)

```nix
# Generated by pleme-linker resolve
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
{
  # Metadata
  generatedAt = "2026-01-15T10:30:00Z";
  resolverVersion = "0.3.0";
  packageCount = 42;

  # Root dependencies (direct deps from package.json)
  rootDependencies = [
    "react@18.2.0"
    "react-dom@18.2.0"
  ];

  # All resolved packages
  packages = {
    "react@18.2.0" = {
      pname = "react";
      version = "18.2.0";
      url = "https://registry.npmjs.org/react/-/react-18.2.0.tgz";
      integrity = "sha512-...";
      dependencies = [ "loose-envify@1.4.0" ];
    };

    "loose-envify@1.4.0" = {
      pname = "loose-envify";
      version = "1.4.0";
      url = "https://registry.npmjs.org/loose-envify/-/loose-envify-1.4.0.tgz";
      integrity = "sha512-...";
      dependencies = [ "js-tokens@4.0.0" ];
      hasBin = true;
    };
  };

  # Workspace packages (local file: dependencies)
  # These are built from source by pleme-linker build-project
  workspacePackages = [
    { name = "@myorg/core"; path = "../packages/core"; }
    { name = "@myorg/ui"; path = "../packages/ui"; }
  ];
}
```

---

## Workspace Configuration (Planned — Phase 2)

### pleme.toml

```toml
# pleme.toml - pleme-linker workspace configuration

[workspace]
name = "my-monorepo"
packages = [
  "packages/libs/*",
  "packages/apps/*/web",
]
build-order = "topological"

[registry]
url = "https://registry.npmjs.org"

[resolve]
include-dev = true
platform = "linux"
arch = "x64"

[nix]
output-dir = "nix/generated"
```

### Workspace Output Structure

```
project-root/
├── pleme.toml
├── package.json
│
├── nix/
│   └── generated/
│       ├── workspace.nix         # Main entry point
│       ├── deps/
│       │   ├── root.nix
│       │   ├── my-core.nix
│       │   └── my-ui.nix
│       └── local/
│           ├── my-core.nix
│           └── my-ui.nix
│
└── packages/
    └── libs/
        ├── my-core/
        └── my-ui/
```

---

## Nix Integration

### web-services.nix Pattern

```nix
mkWebService = { product, service, src, ... }: let
  # Import the generated deps.nix
  deps = import "${src}/deps.nix";

  # Fetch each package individually (cached by Nix/Attic)
  fetchedPackages = lib.mapAttrs (key: pkg:
    pkgs.fetchurl {
      url = pkg.url;
      hash = pkg.integrity;
      name = "${pkg.pname}-${pkg.version}.tgz";
    }
  ) deps.packages;

  # Generate manifest for pleme-linker build
  manifest = pkgs.writeText "manifest.json" (builtins.toJSON {
    packages = lib.mapAttrsToList (key: pkg: {
      pname = pkg.pname;
      version = pkg.version;
      tarball = fetchedPackages.${key};
      dependencies = pkg.dependencies or [];
    }) deps.packages;
  });

  # Build node_modules using pleme-linker (no network, pure)
  nodeModules = pkgs.runCommand "${product}-${service}-node_modules" {
    nativeBuildInputs = [ plemeLinker ];
  } ''
    pleme-linker build \
      --manifest ${manifest} \
      --output $out \
      --node-bin ${pkgs.nodejs_20}/bin/node
  '';
in { ... };
```

---

## CLI Reference

### resolve

Query the npm registry and generate a deps.nix lockfile from package.json.

```bash
pleme-linker resolve [OPTIONS]

Options:
  --project <PATH>      Project root containing package.json (default: .)
  --output <PATH>       Output path for deps.nix (default: <project>/deps.nix)
  --registry <URL>      npm registry URL (default: https://registry.npmjs.org)
  --include-dev         Include devDependencies (default: true)
  --platform <OS>       Target platform filter (default: linux)
```

Handles `workspace:`, `file:`, and `link:` dependencies (recorded as workspace packages in deps.nix). Supports npm aliases (`npm:package@version`). Resolves in parallel with up to 32 concurrent fetches.

### build

Build node_modules from pre-fetched tarballs. Runs inside the Nix sandbox (no network access).

```bash
pleme-linker build [OPTIONS]

Options:
  --manifest <PATH>     Path to manifest JSON (generated by Nix from deps.nix)
  --output <PATH>       Output directory for node_modules
  --node-bin <PATH>     Path to Node.js binary (for postinstall scripts)
```

Uses a pnpm-style content-addressable store (`.pnpm/`) with hoisted symlinks. Root dependencies from package.json are hoisted with priority.

### build-project

Build a complete TypeScript project: node_modules + TypeScript compilation + optional wrapper script.

```bash
pleme-linker build-project [OPTIONS]

Options:
  --manifest <PATH>       Path to manifest JSON
  --project <PATH>        Project source directory
  --output <PATH>         Output directory for the built project
  --node-bin <PATH>       Path to Node.js binary
  --cli-entry <PATH>      CLI entry point relative to dist/ (e.g., "cli.js")
  --bin-name <NAME>       Name for the wrapper binary
  --parent-tsconfig <PATH>  Path to parent tsconfig.json (if project extends it)
  --workspace-dep <NAME=PATH>  Pre-built workspace dependency (repeatable)
  --workspace-src <NAME=MANIFEST=SRC>  Build workspace dep from source (repeatable)
  --use-vite              Use Vite for web app bundling (default: OXC pure Rust bundler)
```

Auto-detects project type: web apps (index.html + src/main.tsx) use OXC or Vite bundling; CLI tools use OXC transpilation. TypeScript compilation is pure Rust via OXC — no Node.js dependency for the compilation step itself.

### build-library

Build a TypeScript library using tsdown (produces dist/).

```bash
pleme-linker build-library [OPTIONS]

Options:
  --manifest <PATH>     Path to manifest JSON
  --src <PATH>          Library source directory (containing package.json, tsdown.config.ts)
  --output <PATH>       Output directory for the built library
  --node-bin <PATH>     Path to Node.js binary
```

### regen

Regenerate deps.nix for a web project. Convenience wrapper around `resolve`.

```bash
pleme-linker regen [OPTIONS]

Options:
  --project-root <PATH>  Path to project root
  --crate2nix <PATH>     Path to crate2nix binary
```

---

## Development Workflow

```bash
# Resolve dependencies (generates deps.nix from package.json)
pleme-linker resolve --project ./my-web-app

# Build with Nix (fetches tarballs via fetchurl, assembles node_modules)
nix build .#my-web-app

# Regenerate deps.nix after changing package.json
pleme-linker regen --project-root ./my-web-app --crate2nix $(which crate2nix)

# Build a TypeScript library
pleme-linker build-library \
  --manifest manifest.json \
  --src ./packages/my-lib \
  --output ./result \
  --node-bin $(which node)

# Build a full TypeScript project with OXC
pleme-linker build-project \
  --manifest manifest.json \
  --project ./my-mcp-server \
  --output ./result \
  --node-bin $(which node) \
  --cli-entry cli.js \
  --bin-name my-server
```

---

## Dependencies

### Cargo.toml

```toml
[dependencies]
# Core
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
clap = { version = "4.5", features = ["derive"] }
chrono = "0.4"

# Archive handling
flate2 = "1.0"
tar = "0.4"

# HTTP client for npm registry
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }

# Semver resolution
semver = "1.0"

# Async runtime
tokio = { version = "1.0", features = ["rt-multi-thread", "macros"] }
futures = "0.3"

# OXC - Oxidation Compiler (pure Rust JS/TS toolchain)
oxc = { version = "0.99.0", features = ["full"] }
oxc_resolver = "6"

# Utilities
regex = "1"
```

---

## Benefits

1. **Zero Lock-in**: Works with any npm-compatible registry
2. **Nix-First**: Designed for Nix, not adapted to it
3. **Simple Config**: One `pleme.toml` vs multiple config files
4. **Monorepo Native**: Workspaces are first-class
5. **Deterministic**: Same input → same Nix expressions → same builds
6. **Cacheable**: Per-package granularity, works with Attic/Cachix
7. **Fast**: Rust implementation, parallel resolution
8. **Transparent**: Generated Nix is readable, debuggable
