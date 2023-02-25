use bindgen::EnumVariation;
use std::env;
use std::io::{stderr, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const GO_LIB_NAME: &str = "go-srp";

fn main() {
    let platform = Platform::from_env();
    let (lib_dir, lib_path) = target_path_for_go_lib(platform);

    println!("cargo:rustc-link-search={}", lib_dir.to_str().unwrap());
    println!("cargo:rustc-link-lib={GO_LIB_NAME}");

    build_go_lib(&lib_path, platform);
    generate_bindings_go_for_lib(&lib_dir)
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum CPUArch {
    X86_64,
    Aarch64,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum Platform {
    Desktop,
    Android(CPUArch),
}

impl Platform {
    fn from_env() -> Platform {
        let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

        if target_os == "android" {
            if target_arch == "x86_64" {
                return Platform::Android(CPUArch::X86_64);
            } else if target_arch == "aarch64" {
                return Platform::Android(CPUArch::Aarch64);
            } else {
                panic!("unsupported android architecture")
            }
        }

        Platform::Desktop
    }
}

fn target_path_for_go_lib(platform: Platform) -> (PathBuf, PathBuf) {
    match platform {
        Platform::Desktop => {
            let lib_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
            (lib_dir.clone(), lib_dir.join(format!("lib{GO_LIB_NAME}.a")))
        }
        Platform::Android(_) => {
            let lib_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
            (
                lib_dir.clone(),
                lib_dir.join(format!("lib{GO_LIB_NAME}.so")),
            )
        }
    }
}

fn build_go_lib(lib_path: &Path, platform: Platform) {
    let mut command = Command::new("go");

    match platform {
        Platform::Desktop => {}
        Platform::Android(arch) => {
            command.env("CGO_ENABLED", "1");
            command.env("GOOS", "android");
            match arch {
                CPUArch::X86_64 => {
                    command.env("GOARCH", "amd64");
                    command.env("CC", env::var("CC_x86_64-linux-android").unwrap());
                }
                CPUArch::Aarch64 => {
                    command.env("GOARCH", "arm64");
                    command.env("CC", env::var("CC_aarch64-linux-android").unwrap());
                }
            };
        }
    }

    command.arg("build");
    command.arg("-o");
    command.arg(lib_path);

    match platform {
        Platform::Desktop => {
            command.arg("-buildmode=c-archive");
            command.arg("lib.go");
            println!("cargo:rerun-if-changed=go/lib.go");
        }
        Platform::Android(_) => {
            command.arg("-buildmode=c-shared");
            command.arg("lib.go");
            println!("cargo:rerun-if-changed=go/lib.go");
        }
    }

    command.current_dir("go");

    let output = command.output().unwrap();
    if !output.status.success() {
        eprint!("Failed to compile go library:");
        stderr()
            .write_all(output.stderr.as_slice())
            .expect("Error write failed");
        panic!("Go lib build failed");
    }
}

fn generate_bindings_go_for_lib(lib_dir: &Path) {
    let header = lib_dir.join("libgo-srp.h");

    let generated_bindings = lib_dir.join("go-srp.rs");

    let bindings = bindgen::Builder::default()
        .header(header.to_str().unwrap())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .derive_debug(false)
        .impl_debug(false)
        .default_enum_style(EnumVariation::Rust {
            non_exhaustive: false,
        })
        .generate()
        .expect("Unable to generate go lib bindings");

    bindings
        .write_to_file(generated_bindings)
        .expect("Failed to write bindings to file");
}