//! Build script: the build number and the Windows icon/version resource.
//!
//! The icon is the OpenWX brand icon (`brand/openwx.ico`, made by `brand/tools/openwx_brand.py`).

use std::{fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=BUILD_NUMBER");
    // Build number: same version, new build (bumped by `openwx-release bump`).
    let build = fs::read_to_string("BUILD_NUMBER").ok().and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(1);
    println!("cargo:rustc-env=CWX_BUILD={build}");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let ico = manifest_dir.join("../brand/openwx.ico");
    println!("cargo:rerun-if-changed={}", ico.display());

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && ico.exists() {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico.to_str().unwrap());
        res.set("ProductName", "OpenWX Broadcasting");
        res.set("FileDescription", "OpenWX Broadcasting");
        res.set("CompanyName", "OpenWX Broadcasting contributors");
        res.set("LegalCopyright", "© OpenWX Broadcasting contributors");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed Windows resources: {e}");
        }
    }
}
