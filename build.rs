use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_rc_exe() -> Option<PathBuf> {
    if let Ok(path) = env::var("RC") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }
    if Command::new("rc.exe").arg("/?").output().is_ok() {
        return Some(PathBuf::from("rc.exe"));
    }
    let kits_bin = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    if kits_bin.is_dir() {
        let mut versions: Vec<PathBuf> = std::fs::read_dir(kits_bin)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path().join("x64").join("rc.exe"))
            .filter(|p| p.is_file())
            .collect();
        versions.sort();
        if let Some(last) = versions.pop() {
            return Some(last);
        }
    }
    None
}

fn main() {
    println!("cargo:rerun-if-changed=assets/icons/app.rc");
    println!("cargo:rerun-if-changed=assets/icons/sensors-gorilla.ico");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let rc = find_rc_exe().expect("Windows SDK rc.exe not found to embed app icon resource");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let res_path = out_dir.join("sensors-gorilla.res");
    let status = Command::new(&rc)
        .args(["/nologo", "/fo"])
        .arg(&res_path)
        .arg("assets/icons/app.rc")
        .status()
        .expect("failed to execute rc.exe");
    assert!(status.success(), "rc.exe failed to compile assets/icons/app.rc");
    println!("cargo:rustc-link-arg-bins={}", res_path.display());
}
