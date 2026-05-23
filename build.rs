fn main() {
    // Increase main thread stack to 128MB on macOS
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-arg=-Wl,-stack_size,0x8000000");

    // Embed Windows application icon
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icons/zk-ui.ico");
        res.compile().unwrap();
    }
}
