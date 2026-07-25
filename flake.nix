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

            # `cargo xwin build --target aarch64-pc-windows-msvc`; lld/llvm
            # supply lld-link and llvm-lib, clang+cmake build aws-lc-sys.
            pkgs.cargo-xwin
            pkgs.lld
            pkgs.llvm
            pkgs.clang
            pkgs.cmake
          ]
          # Linux runtime deps for winit/wgpu (X11/Wayland, libxkbcommon, a
          # Vulkan/GL loader). Starting point — confirm the set when the Linux
          # port begins.
          # ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          #   pkgs.libxkbcommon
          #   pkgs.wayland
          #   pkgs.libGL
          #   pkgs.vulkan-loader
          #   pkgs.xorg.libX11
          #   pkgs.xorg.libXcursor
          #   pkgs.xorg.libXi
          #   pkgs.xorg.libXrandr
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
