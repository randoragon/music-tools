# Based on https://dev.to/misterio/how-to-package-a-rust-app-using-nix-3lh3
{
  description = "A collection of programs for managing and enhancing local music listening experience";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };
  outputs = { self, nixpkgs }:
    let
      supportedSystems = [ "x86_64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      pkgsFor = nixpkgs.legacyPackages;
      manifest = (nixpkgs.lib.importTOML ./Cargo.toml).package;
      music-tools = pkgs: pkgs.rustPlatform.buildRustPackage {
        pname = manifest.name;
        version = manifest.version;
        cargoLock.lockFile = ./Cargo.lock;
        src = pkgs.lib.cleanSource ./.;
      };
    in {
      packages = forAllSystems (system: {
        default = music-tools pkgsFor.${system};
      });

      devShells = forAllSystems (system: {
        default = pkgsFor.${system}.mkShell {
          inputsFrom = [ (music-tools pkgsFor.${system}) ];
          buildInputs = with pkgsFor.${system}; [
            clippy
            rustc
            gdb
          ];
        };
      });
    };
}
