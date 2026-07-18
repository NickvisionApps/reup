fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if !["windows", "macos", "linux"].contains(&target_os.as_str()) {
        panic!("armoire does not support target_os = \"{target_os}\"");
    }
}
