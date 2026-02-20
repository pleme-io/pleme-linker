# pleme-linker

Nix-native JavaScript package manager. Resolves npm dependencies and builds `node_modules` hermetically for Nix builds — no npm/pnpm/yarn needed in the sandbox.

## Key Idea

The npm registry is just HTTP. We query it directly, output Nix expressions (the `deps.nix` file IS the lockfile), and let Nix's `fetchurl` handle downloading. Building `node_modules` is pure — no network access required.

## Commands

| Command | Purpose |
|---------|---------|
| `resolve` | Query npm registry, resolve versions, generate `deps.nix` |
| `build` | Assemble `node_modules` from pre-fetched tarballs (no network) |
| `build-project` | Full TypeScript project build (node_modules + OXC/Vite + wrapper) |
| `build-library` | Build TypeScript library via tsdown (produces `dist/`) |
| `regen` | Regenerate `deps.nix` for a web project |

## Quick Start

```bash
# Install via Nix flake
nix build github:pleme-io/pleme-linker

# Resolve dependencies (generates deps.nix from package.json)
pleme-linker resolve --project ./my-app

# The generated deps.nix is consumed by your Nix build
```

## How It Works

```
package.json ──→ pleme-linker resolve ──→ deps.nix (Nix expression)
                      │                        │
                      │ (queries npm registry)  │ (consumed by Nix)
                      ▼                        ▼
               npm registry            nix build .#my-app
                                            │
                                            │ fetchurl (per-package, cached)
                                            ▼
                                  pleme-linker build ──→ node_modules/
                                  (pure, no network)
```

1. **`resolve`** reads `package.json`, queries the npm registry, resolves the full dependency tree, and writes `deps.nix`
2. **Nix** reads `deps.nix`, fetches each tarball individually via `fetchurl` (cached by Nix store / Attic / Cachix)
3. **`build`** takes the fetched tarballs and assembles a pnpm-style `node_modules` with hoisted symlinks — no network access needed

## Nix Integration

```nix
# In your flake, import deps.nix and feed tarballs to pleme-linker build
let
  deps = import "${src}/deps.nix";
  fetchedPackages = lib.mapAttrs (key: pkg:
    pkgs.fetchurl { url = pkg.url; hash = pkg.integrity; }
  ) deps.packages;

  manifest = pkgs.writeText "manifest.json" (builtins.toJSON {
    packages = lib.mapAttrsToList (key: pkg: {
      inherit (pkg) pname version;
      tarball = fetchedPackages.${key};
      dependencies = pkg.dependencies or [];
    }) deps.packages;
  });

  nodeModules = pkgs.runCommand "node_modules" {
    nativeBuildInputs = [ pleme-linker ];
  } ''
    pleme-linker build --manifest ${manifest} --output $out --node-bin ${nodejs}/bin/node
  '';
in { ... }
```

## TypeScript Compilation

`build-project` uses [OXC](https://oxc.rs/) (Oxidation Compiler) for pure Rust TypeScript compilation — no Node.js needed for the compilation step. Web apps can optionally use Vite (`--use-vite`).

## License

MIT
