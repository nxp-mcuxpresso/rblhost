// Copyright 2025 NXP
//
// SPDX-License-Identifier: BSD-3-Clause
#[cfg(feature = "c_api")]
use std::env;

#[cfg(feature = "c_api")]
fn generate_c_bindings() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let config = cbindgen::Config::from_file("cbindgen.toml").unwrap();

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file("include/mboot.h");
}

#[cfg(not(feature = "c_api"))]
fn generate_c_bindings() {
    // Do nothing if c_api feature is not enabled
    println!("C API bindings generation skipped (feature not enabled)");
}

#[cfg(feature = "csharp")]
fn generate_csharp_bindings() {
    csbindgen::Builder::default()
        .input_extern_file("src/c_api.rs")
        .csharp_dll_name("mboot")
        .csharp_namespace("RblHost")
        .csharp_class_name("NativeMethods")
        .csharp_class_accessibility("public")
        .csharp_use_nint_types(false)
        .always_included_types([
            "CGetPropertyResponse",
            "CReadMemoryResponse",
            "CFlashReadOnceResponse",
            "CTrustProvisioningResponse",
            "CKeyStoreResponse",
            "CProtocol",
        ])
        .generate_csharp_file("bindings/csharp/NativeMethods.g.cs")
        .expect("Unable to generate C# bindings");
}

#[cfg(not(feature = "csharp"))]
fn generate_csharp_bindings() {
    // Do nothing if csharp feature is not enabled
    println!("C# bindings generation skipped (feature not enabled)");
}

fn main() {
    generate_c_bindings();
    generate_csharp_bindings();
}
