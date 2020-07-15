use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

fn main() {
    let pylon_root = Path::new(env!("PYLON_ROOT"));
    let pylon_include = pylon_root.join("include");

    let pylon_libs = if env::var("CARGO_CFG_WINDOWS").is_ok() {
        pylon_root.join("lib").join("x64")
    } else if env::var("CARGO_CFG_UNIX").is_ok() {
        pylon_root.join("/lib64")
    } else {
        panic!("Unexpected target platform");
    };

    println!("cargo:rustc-link-lib=pylonc");
    println!("cargo:rustc-link-lib=pylonbase_v6_1");
    println!("cargo:rustc-link-lib=pylonutility_v6_1");

    // find all additional libs in the pylon libs dir. They may have different names, so glob them
    let additional = &[
        "libGenApi_gcc",
        "libGCBase_gcc",
        "libLog_gcc",
        "libMathParser_gcc",
        "libXmlParser_gcc",
        "libNodeMapData_gcc",
    ];
    if let Ok(paths) = fs::read_dir(&pylon_libs) {
        for path in paths {
            if let Ok(p) = path {
                let file = p.file_name().into_string().unwrap();
                if additional.iter().any(|lib| file.starts_with(lib)) {
                    println!(
                        "cargo:rustc-link-lib={}",
                        file.trim_start_matches("lib").trim_end_matches(".dll")
                    );
                }
            }
        }
    };

    println!("cargo:rustc-link-search={}", pylon_libs.to_str().unwrap());

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        .clang_arg(format!("-I{}", pylon_include.to_str().unwrap()))
        .clang_arg(format!("-L{}", pylon_libs.to_str().unwrap()))
        .default_enum_style(bindgen::EnumVariation::Rust)
        .derive_default(false)
        .derive_debug(false)
        .opaque_type("PYLON_DEVICE_HANDLE")
        .opaque_type("PYLON_STREAMGRABBER_HANDLE")
        .opaque_type("PYLON_STREAMBUFFER_HANDLE")
        .opaque_type("PYLON_WAITOBJECT_HANDLE")
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
