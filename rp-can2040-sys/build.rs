use std::env;
use std::path::PathBuf;

fn main() {
    let is_rp2350 = env::var("CARGO_FEATURE_RP2350").is_ok();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    link_library(is_rp2350);
    provide_bindings(is_rp2350, &out_dir);
}

fn link_library(is_rp2350: bool) {
    #[cfg(feature = "build-from-source")]
    build_library_from_source(is_rp2350);

    #[cfg(not(feature = "build-from-source"))]
    use_prebuilt_library(is_rp2350);

    println!("cargo:rustc-link-lib=static=can2040");
}

// Compiles can2040.c via CMake using the pico-sdk build infrastructure.
//
// The cc crate can't be used here because pico-sdk generates headers at
// configure time (pico/version.h, etc.) that can2040.c depends on.
#[cfg(feature = "build-from-source")]
fn build_library_from_source(is_rp2350: bool) {
    let pico_sdk = env::var("PICO_SDK_PATH")
        .expect("PICO_SDK_PATH must be set when using the build-from-source feature");

    // "pico" = RP2040, "pico2" = RP2350; selects the linker script and chip defines.
    let pico_board = if is_rp2350 { "pico2" } else { "pico" };

    let dst = cmake::Config::new(".")
        .define("PICO_SDK_PATH", &pico_sdk)
        .define("PICO_BOARD", pico_board)
        // CMake 4.x requires absolute paths for cross-compilation tools; which() resolves them.
        .define("CMAKE_C_COMPILER", &which("arm-none-eabi-gcc"))
        .define("CMAKE_CXX_COMPILER", &which("arm-none-eabi-g++"))
        .define("CMAKE_ASM_COMPILER", &which("arm-none-eabi-gcc")) // pico-sdk ASM goes through GCC
        // Prevents CMake's compiler sanity check from trying to link a bare-metal executable.
        .define("CMAKE_TRY_COMPILE_TARGET_TYPE", "STATIC_LIBRARY")
        // pico-sdk post-processes boot_stage2 with objdump/objcopy; must be the ARM variants.
        .define("CMAKE_OBJDUMP", &which("arm-none-eabi-objdump"))
        .define("CMAKE_OBJCOPY", &which("arm-none-eabi-objcopy"))
        .build();

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rerun-if-changed=can2040/src/can2040.c");
    println!("cargo:rerun-if-changed=CMakeLists.txt");
}

#[cfg(not(feature = "build-from-source"))]
fn use_prebuilt_library(is_rp2350: bool) {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let target_name = if is_rp2350 { "rp2350" } else { "rp2040" };
    println!("cargo:rustc-link-search=native={}/lib/{}", manifest_dir.display(), target_name);
}

// Writes bindings to OUT_DIR so lib.rs can unconditionally include! them
// regardless of whether they were generated or copied from prebuilt.
// _is_rp2350 is only used when the bindgen feature is active; prefix suppresses
// the unused-variable warning on the default (prebuilt) path.
fn provide_bindings(_is_rp2350: bool, out_dir: &PathBuf) {
    #[cfg(feature = "bindgen")]
    generate_bindings(_is_rp2350, out_dir);

    #[cfg(not(feature = "bindgen"))]
    copy_prebuilt_bindings(out_dir);
}

#[cfg(feature = "bindgen")]
fn generate_bindings(is_rp2350: bool, out_dir: &PathBuf) {
    // Without an explicit target, clang defaults to x86-64, producing wrong type sizes.
    // Note: the bindings are currently identical between RP2040 and RP2350 since
    // can2040.h uses only fixed-width types with no arch-specific code.
    let target = if is_rp2350 {
        "--target=thumbv8m.main-none-eabihf"
    } else {
        "--target=thumbv6m-none-eabi"
    };

    bindgen::Builder::default()
        .clang_arg(target)
        .header("can2040/src/can2040.h")
        .derive_debug(true)
        .use_core() // emit core:: types so bindings work in no_std crates
        .layout_tests(false) // layout tests require std::test, unavailable on embedded targets
        .generate()
        .expect("Failed to generate bindings")
        .write_to_file(out_dir.join("can2040_bindings.rs"))
        .expect("Failed to write bindings");

    println!("cargo:rerun-if-changed=can2040/src/can2040.h");
}

#[cfg(not(feature = "bindgen"))]
fn copy_prebuilt_bindings(out_dir: &PathBuf) {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let prebuilt = manifest_dir.join("src").join("bindings.rs");
    std::fs::copy(&prebuilt, out_dir.join("can2040_bindings.rs"))
        .unwrap_or_else(|_| panic!("Prebuilt bindings not found at {}", prebuilt.display()));
}

// CMake 4.x requires absolute paths for cross-compilation tools when targeting
// a bare-metal (Generic) system; relative names are rejected at configure time.
#[cfg(feature = "build-from-source")]
fn which(tool: &str) -> String {
    let out = std::process::Command::new("which")
        .arg(tool)
        .output()
        .unwrap_or_else(|_| panic!("{tool} not found in PATH"));
    assert!(out.status.success(), "{tool} not found in PATH");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}
