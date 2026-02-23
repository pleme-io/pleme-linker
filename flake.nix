{
  description = "pleme-linker - Nix-native JavaScript package manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/d6c71932130818840fc8fe9509cf50be8c64634f";
    crate2nix = {
      url = "github:nix-community/crate2nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, crate2nix }:
    let
      supportedSystems = [ "aarch64-darwin" "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          isDarwin = pkgs.stdenv.isDarwin;

          # Darwin-specific build inputs for Rust crates that need system frameworks
          darwinBuildInputs = pkgs.lib.optionals isDarwin (
            [ pkgs.libiconv ]
            ++ (if pkgs ? apple-sdk
                then [ pkgs.apple-sdk ]
                else pkgs.lib.optionals (pkgs ? darwin) (
                  with pkgs.darwin.apple_sdk.frameworks; [
                    Security
                    SystemConfiguration
                    CoreFoundation
                  ]
                ))
          );

          cargoNix = pkgs.callPackage ./Cargo.nix {
            defaultCrateOverrides = pkgs.defaultCrateOverrides // {
              ring = attrs: {
                nativeBuildInputs = (attrs.nativeBuildInputs or [ ]) ++ [
                  pkgs.cmake
                  pkgs.perl
                ];
                buildInputs = (attrs.buildInputs or [ ]) ++ darwinBuildInputs;
              };
              pleme-linker = attrs: {
                nativeBuildInputs = (attrs.nativeBuildInputs or [ ]) ++ [
                  pkgs.cmake
                  pkgs.perl
                  pkgs.git
                ];
                buildInputs = (attrs.buildInputs or [ ]) ++ darwinBuildInputs;
              };
            };
          };
        in
        {
          default = cargoNix.rootCrate.build;
          pleme-linker = cargoNix.rootCrate.build;
        }
      );

      apps = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          crate2nix-pkg = crate2nix.packages.${system}.default;
        in
        {
          regen = {
            type = "app";
            program = toString (pkgs.writeShellScript "regen-cargo-nix" ''
              ${crate2nix-pkg}/bin/crate2nix generate
            '');
          };
        }
      );
    };
}
