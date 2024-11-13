{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix.url = "github:nix-community/fenix/monthly";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nixpkgs, flake-utils, ... }@inputs: flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ inputs.fenix.overlays.default ];
      };
      buildDependencies = with pkgs; [];
      runtimeDependencies = with pkgs; [
          mold
          lld
          sccache
          pkg-config
      ];
      components = [
          "rustc"
          "cargo"
          "clippy"
          "rustfmt"
          "rust-analyzer"
          "rust-src"
          "llvm-tools-preview"
          # Nightly
          "rustc-codegen-cranelift-preview"
          "miri"
      ];
      nightly = pkgs.fenix.complete.withComponents components;
      stable = pkgs.fenix.stable.withComponents ( nixpkgs.lib.sublist 0 (builtins.length components - 3) components );
    
    in {
      devShells.default = pkgs.mkShell rec {
        nativeBuildInputs = with pkgs; [
          nightly
          # stable
          
          fenix.targets.x86_64-unknown-linux-gnu.latest.rust-std
          
          rustup
          cargo-msrv

          openssl.dev
        ] ++ buildDependencies;
        
        buildInputs = runtimeDependencies;

        RUST_SRC_PATH = "${pkgs.fenix.complete.rust-src}/lib/rustlib/src/rust/library";
        RUSTC_WRAPPER = "sccache";
        RUSTFLAGS = "-Ctarget-cpu=native -Clink-arg=-fuse-ld=mold";
        MSRVFLAGS = "-Clink-arg=-fuse-ld=mold"; # RUSTFLAGS=$MSRVFLAGS cargo msrv

        LD_LIBRARY_PATH = with pkgs; lib.makeLibraryPath nativeBuildInputs;
      };
    });
}
