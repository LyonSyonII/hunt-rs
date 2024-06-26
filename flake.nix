{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix.url = "github:nix-community/fenix";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nixpkgs, flake-utils, ... }@inputs: flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ inputs.fenix.overlays.default ];
      };
    in
    {
      devShells.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          sccache
          lld
          mold
          rust-analyzer-nightly

          fenix.complete.rustc
          fenix.complete.cargo
          fenix.complete.clippy
          fenix.complete.rustfmt
          fenix.complete.miri
          fenix.complete.rust-src
          fenix.complete.rustc-codegen-cranelift-preview
          fenix.complete.llvm-tools-preview
        ];
        RUST_SRC_PATH = "${pkgs.fenix.complete.rust-src}/lib/rustlib/src/rust/library";
        RUSTFLAGS="-Zcodegen-backend=llvm";
      };
    });
}