use std::path::PathBuf;
use std::process::Command;

fn main() {
    embed_test_manifest();
    tauri_build::build()
}

/// Link a Common-Controls-v6 manifest into *test* binaries on windows-gnu.
///
/// tauri-build embeds this manifest into the app binary, but cargo test
/// binaries get none. Without it the loader binds the System32 comctl32.dll
/// (v5.82), which lacks `TaskDialogIndirect` (imported via muda/tauri), and
/// every test executable dies at load with STATUS_ENTRYPOINT_NOT_FOUND.
fn embed_test_manifest() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("gnu")
    {
        return;
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let manifest_path = out_dir.join("test-manifest.xml");
    let rc_path = out_dir.join("test-manifest.rc");
    let obj_path = out_dir.join("test-manifest.o");

    let manifest = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0" processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
</assembly>
"#;
    std::fs::write(&manifest_path, manifest).expect("write manifest");
    // Resource type 24 = RT_MANIFEST, ID 1 = process default manifest.
    std::fs::write(
        &rc_path,
        format!("1 24 \"{}\"\n", manifest_path.display().to_string().replace('\\', "/")),
    )
    .expect("write rc");

    let status = Command::new("windres")
        .arg(&rc_path)
        .arg("-o")
        .arg(&obj_path)
        .status();
    match status {
        Ok(s) if s.success() => {
            println!("cargo:rustc-link-arg-tests={}", obj_path.display());
        }
        // windres missing or failing only breaks `cargo test`, not the app
        // build, so warn instead of aborting.
        Ok(s) => println!("cargo:warning=windres exited with {s}; test binaries will not have a Common-Controls manifest"),
        Err(e) => println!("cargo:warning=windres unavailable ({e}); test binaries will not have a Common-Controls manifest"),
    }
}
