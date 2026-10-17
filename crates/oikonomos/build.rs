fn main() {
    println!("cargo:rerun-if-changed=../../web/build");
    println!("cargo:rerun-if-env-changed=OIKONOMOS_BUILD_ID");
    let build = std::env::var("OIKONOMOS_BUILD_ID").unwrap_or_else(|_| std::env::var("CARGO_PKG_VERSION").unwrap());
    println!("cargo:rustc-env=OIKONOMOS_BUILD_ID={build}");
}
