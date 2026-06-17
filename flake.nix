{
  description = "Yoke — configuration software for the QuadStick.";

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url  = "github:numtide/flake-utils";
    fenix = {
      url            = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # Single source of truth: rust-toolchain.toml, pinned to an explicit
        # version so the fetched channel manifest is immutable. When bumping
        # the version there, the first `nix develop` prints the new sha256 in a
        # hash-mismatch error; substitute the printed hash here.
        rustToolchain = fenix.packages.${system}.fromToolchainFile {
          file   = ./rust-toolchain.toml;
          sha256 = "sha256-mvUGEOHYJpn3ikC5hckneuGixaC+yGrkMM/liDIDgoU=";
        };
      in {
        devShells.default = pkgs.mkShell {
          name = "yoke";

          packages = [
            rustToolchain
            pkgs.trunk
            pkgs.pkg-config
          ]
          # Linux runtime deps for Tauri's webview. Uncomment when the
          # Linux port begins.
          # ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          #   pkgs.webkitgtk_4_1
          #   pkgs.libayatana-appindicator
          # ]
          ;

          shellHook = ''
            echo "Yoke devShell ready."
            echo "  rustc: $(rustc --version)"
            echo "  trunk: $(trunk --version 2>/dev/null | head -n1)"
          '';
        };
      });
}
