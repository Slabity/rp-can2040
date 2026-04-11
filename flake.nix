{
  description = "Rust wrapper and FFI bindings for the can2040 software CAN bus implementation, targeting RP2040 and RP2350";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "llvm-tools-preview" ];
          targets = [ "thumbv6m-none-eabi" "thumbv8m.main-none-eabihf" ];
        };
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.gcc-arm-embedded   # arm-none-eabi-gcc, picked up by the pico-sdk CMake toolchain
            pkgs.cmake
            pkgs.ninja
            pkgs.pico-sdk
            pkgs.llvmPackages.libclang  # required by bindgen
            pkgs.python3                # required by pico-sdk CMake (boot_stage2 generation)
            pkgs.flip-link          # linker wrapper used by .cargo/config.toml
            pkgs.probe-rs-tools     # provides `probe-rs` for flashing/debugging
          ];

          # Path to the pico-sdk root, used by CMakeLists.txt and build.rs
          PICO_SDK_PATH = "${pkgs.pico-sdk}/lib/pico-sdk";

          # Required by the bindgen crate to locate libclang
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        };
      }
    );
}
