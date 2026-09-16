fn main() {
    configure_linux_windowing();
    yolite_lib::run()
}

#[cfg(target_os = "linux")]
fn configure_linux_windowing() {
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
    {
        // Work around WebKitGTK/Wayland DMA-BUF crashes on some GPU stacks.
        unsafe {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_linux_windowing() {}
