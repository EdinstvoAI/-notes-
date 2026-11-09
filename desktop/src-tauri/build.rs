use std::{env, path::PathBuf, process::Command};

fn checked(command: &mut Command) -> String {
    let output = command.output().expect("запуск системного компилятора");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).expect("вывод компилятора UTF-8")
}

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=native/MacBridge.swift");
        let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
        let object = out.join("notes_macos.o");
        let target = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
            Ok("aarch64") => "arm64-apple-macosx14.0", Ok("x86_64") => "x86_64-apple-macosx14.0", _ => panic!("macOS architecture"),
        };
        let info = checked(Command::new("xcrun").args(["swiftc", "-print-target-info", "-target", target]));
        let info: serde_json::Value = serde_json::from_str(&info).unwrap();
        for path in info["paths"]["runtimeLibraryPaths"].as_array().unwrap() {
            println!("cargo:rustc-link-search=native={}", path.as_str().unwrap());
        }
        checked(Command::new("xcrun").args(["swiftc", "-swift-version", "5", "-parse-as-library", "-emit-object", "-target", target, "-module-cache-path"])
            .arg(out.join("swift-cache")).arg("native/MacBridge.swift").arg("-o").arg(&object));
        checked(Command::new("xcrun").args(["libtool", "-static", "-o"]).arg(out.join("libnotes_macos.a")).arg(&object));
        println!("cargo:rustc-link-search=native={}", out.display());
        println!("cargo:rustc-link-lib=static=notes_macos");
        // Mach-O хранит фреймворки Swift в LC_LINKER_OPTION; ld подхватывает их из объекта.
        println!("cargo:rustc-link-lib=dylib=swiftCore");
        println!("cargo:rustc-link-lib=dylib=swift_Concurrency");
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
    tauri_build::build()
}
