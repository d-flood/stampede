fn main() {
    println!("cargo:rerun-if-changed=../../web/build");
    println!("cargo:rerun-if-env-changed=STAMPEDE_BUILD_ID");
    let build = std::env::var("STAMPEDE_BUILD_ID").unwrap_or_else(|_| std::env::var("CARGO_PKG_VERSION").unwrap());
    println!("cargo:rustc-env=STAMPEDE_BUILD_ID={build}");
    println!("cargo:rerun-if-env-changed=STAMPEDE_COMMIT");
    println!("cargo:rustc-env=STAMPEDE_COMMIT={}", std::env::var("STAMPEDE_COMMIT").unwrap_or_default());
}
