use bindgen;
use std::env;
use std::path::{PathBuf};
//use std::process::Command;

fn main() {
    let pylon_root = env!("PYLON_ROOT");
    let pylon_include = format!("{}/include", pylon_root);
    let pylon_libs = format!("{}/lib64", pylon_root);

    println!("cargo:rustc-link-lib=pylonc");
    println!("cargo:rustc-link-lib=pylonbase");
    println!("cargo:rustc-link-lib=pylonutility");
    println!("cargo:rustc-link-lib=GenApi_gcc_v3_1_Basler_pylon");
    println!("cargo:rustc-link-lib=GCBase_gcc_v3_1_Basler_pylon");
    println!("cargo:rustc-link-lib=Log_gcc_v3_1_Basler_pylon");
    println!("cargo:rustc-link-lib=MathParser_gcc_v3_1_Basler_pylon");
    println!("cargo:rustc-link-lib=XmlParser_gcc_v3_1_Basler_pylon");
    println!("cargo:rustc-link-lib=NodeMapData_gcc_v3_1_Basler_pylon");
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