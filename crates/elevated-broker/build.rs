#![forbid(unsafe_code)]

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=undelete-master-broker.manifest");
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    let manifest = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo supplies CARGO_MANIFEST_DIR"),
    )
    .join("undelete-master-broker.manifest");
    println!("cargo:rustc-link-arg-bin=undelete-master-broker=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-bin=undelete-master-broker=/MANIFESTUAC:NO");
    println!(
        "cargo:rustc-link-arg-bin=undelete-master-broker=/MANIFESTINPUT:{}",
        manifest.display()
    );
}
