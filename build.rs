use std::{env, fs, path::PathBuf, process::Command};
fn main() {
    let kits =
        PathBuf::from(env::var_os("ProgramFiles(x86)").unwrap()).join(r"Windows Kits\10\bin");
    let mut paths: Vec<_> = fs::read_dir(kits)
        .unwrap()
        .flatten()
        .map(|e| e.path().join(r"x64\rc.exe"))
        .filter(|p| p.exists())
        .collect();
    paths.sort();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("app.res");
    assert!(Command::new(paths.last().expect("Windows SDK required"))
        .arg("/nologo")
        .arg("/c65001")
        .arg("/fo")
        .arg(&out)
        .arg("ui.rc")
        .status()
        .unwrap()
        .success());
    println!("cargo:rustc-link-arg={}", out.display());
    println!("cargo:rerun-if-changed=ui.rc");
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=kagari.ico");
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rustc-link-arg=/MANIFEST:NO");
}
