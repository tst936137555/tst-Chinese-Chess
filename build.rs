fn main() {
    println!("cargo:rerun-if-env-changed=PIKAFISH_LIB_DIR");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if os == "windows" {
        windows_icon();
    }
    if os == "ios" || os == "android" {
        let dir = std::env::var("PIKAFISH_LIB_DIR")
            .expect("移动端需先构建皮卡鱼静态库并设置 PIKAFISH_LIB_DIR，见 README.md");
        println!("cargo:rerun-if-changed={dir}/libpikafish.a");
        println!("cargo:rustc-link-search=native={dir}");
        println!("cargo:rustc-link-lib=static=pikafish");
        if os == "ios" {
            println!("cargo:rustc-link-lib=c++");
            println!("cargo:rustc-link-lib=framework=AVFoundation");
        } else {
            println!("cargo:rustc-link-lib=static=c++_static");
            println!("cargo:rustc-link-lib=static=c++abi");
        }
    }
}

fn windows_icon() {
    use std::{path::PathBuf, process::Command};
    println!("cargo:rerun-if-changed=assets/icons/app.ico");
    println!("cargo:rerun-if-env-changed=WINDRES");
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let input = out.join("app-icon.rc");
    let icon = root
        .join("assets/icons/app.ico")
        .to_string_lossy()
        .replace('\\', "/");
    std::fs::write(
        &input,
        format!("#pragma code_page(65001)\n1 ICON \"{icon}\"\n"),
    )
    .unwrap();
    let gnu = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default() == "gnu";
    let output = out.join(if gnu { "app-icon.o" } else { "app-icon.res" });
    let status = if gnu {
        Command::new(std::env::var_os("WINDRES").unwrap_or_else(|| "windres".into()))
            .args([
                "--input-format=rc",
                "--output-format=coff",
                "--codepage=65001",
            ])
            .arg(&input)
            .arg(&output)
            .status()
    } else {
        let mut rc = PathBuf::from("rc.exe");
        if let Some(programs) = std::env::var_os("ProgramFiles(x86)") {
            let bin = PathBuf::from(programs).join("Windows Kits/10/bin");
            let mut versions: Vec<_> = std::fs::read_dir(bin)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|p| p.path())
                .collect();
            versions.sort();
            if let Some(found) = versions
                .into_iter()
                .rev()
                .map(|p| p.join("x64/rc.exe"))
                .find(|p| p.is_file())
            {
                rc = found;
            }
        }
        Command::new(rc)
            .arg("/nologo")
            .arg("/fo")
            .arg(&output)
            .arg(&input)
            .status()
    }
    .expect("需要 Windows SDK rc.exe 或 GNU windres 编译桌面图标");
    assert!(status.success(), "桌面图标资源编译失败");
    println!("cargo:rustc-link-arg-bin=tst-xiangqi={}", output.display());
}
