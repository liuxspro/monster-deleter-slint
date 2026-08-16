fn main() {
    slint_build::compile("ui/app-window.slint").expect("Slint build failed");

    // 把 ui/assets/icon.ico 作为资源嵌入 exe：
    // 资源管理器里 exe 文件的图标、右键菜单的图标（注册表 Icon 值指向 exe 第 0 号图标）都会用它。
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winres::WindowsResource::new()
            .set_icon("ui/assets/icon.ico")
            .compile()
            .expect("Failed to embed icon resource");
    }
}
