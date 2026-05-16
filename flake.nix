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

        # Single source of truth: rust-toolchain.toml.
        # First `nix develop` after edits will print the real sha256 if this
        # placeholder is wrong; substitute the printed hash in place.
        rustToolchain = fenix.packages.${system}.fromToolchainFile {
          file   = ./rust-toolchain.toml;
          sha256 = "sha256-gh/xTkxKHL4eiRXzWv8KP7vfjSk61Iq48x47BEDFgfk=";
        };
      in {
        devShells.default = pkgs.mkShell {
          name = "yoke";

          packages = [
            rustToolchain
            pkgs.trunk
            pkgs.cargo-tauri
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
            echo "  cargo-tauri: $(cargo tauri --version 2>/dev/null || echo 'not on PATH')"
          '';
        };
      });
}
