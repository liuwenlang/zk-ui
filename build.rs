fn main() {
    // Increase main thread stack to 128MB on macOS
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-arg=-Wl,-stack_size,0x8000000");
}
