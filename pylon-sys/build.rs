use bindgen;
use std::env;
use std::path::PathBuf;
use std::fs;

fn main() {
    let pylon_root = env!("PYLON_ROOT");
    let pylon_include = format!("{}/include", pylon_root);
    let pylon_libs = format!("{}/lib64", pylon_root);

    println!("cargo:rustc-link-lib=pylonc");
    println!("cargo:rustc-link-lib=pylonbase");
    println!("cargo:rustc-link-lib=pylonutility");

    // find all additional libs in the pylon libs dir. They may have different names, so glob them
    let additional = &["libGenApi_gcc", "libGCBase_gcc", "libLog_gcc", "libMathParser_gcc", "libXmlParser_gcc", "libNodeMapData_gcc" ];
    if let Ok(paths) = fs::read_dir(&pylon_libs) {
        for path in paths {
            if let Ok(p) = path {
                let file = p.file_name().into_string().unwrap();
                if additional.iter().any(|lib| file.starts_with(lib)) {
                    println!("cargo:rustc-link-lib={}", file.trim_start_matches("lib").trim_end_matches(".so"));
                }
            }
        }
    };

    println!("cargo:rustc-link-search={}", pylon_libs);

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        .clang_arg(format!("-I{}", pylon_include))
        .clang_arg(format!("-L{}", pylon_libs))
        .default_enum_style(bindgen::EnumVariation::Rust)
        .derive_default(true)
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