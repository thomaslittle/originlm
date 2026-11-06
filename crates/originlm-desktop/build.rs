//! SPIKE: compiles `nsh/ProviderCard.tsx` to JS with NSH's bundler, as
//! `nsh build` does. The result is loaded as *source* at run time
//! (`Bundle::source`); bytecode loading is unsafe and this workspace
//! forbids unsafe code.

use std::path::PathBuf;

fn main() {
    let dir =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir")).join("nsh");
    println!("cargo:rerun-if-changed={}", dir.display());

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("out dir")).join("provider-card.js");
    let bundle = nsh_bundler::bundle(&nsh_bundler::BundleOptions {
        entry: dir.join("ProviderCard.tsx"),
        minify: false,
        split: false,
        ui_sets: Vec::new(),
    })
    .unwrap_or_else(|error| fail(&error.plain));

    std::fs::write(&out, bundle.code)
        .unwrap_or_else(|error| fail(&format!("write {}: {error}", out.display())));
}

fn fail(message: &str) -> ! {
    eprintln!("nsh spike bundle: {message}");
    std::process::exit(1);
}
