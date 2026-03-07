use std::env::var;

fn main() {
    println!("cargo:rerun-if-env-changed=CARBURETOR_TARGET");

    println!("cargo:rustc-check-cfg=cfg(for_backend)");
    println!("cargo:rustc-check-cfg=cfg(for_client)");
    match var("CARBURETOR_TARGET").as_deref() {
        Ok("client") => {
            println!("cargo:rustc-cfg=for_client");
        }
        _ => {
            // Defaults to backend
            println!("cargo:rustc-cfg=for_backend");
        }
    }
}
