// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod context_menu;
mod trash;

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use rodio::source::Source;

slint::include_modules!();

/// 音频输出设备句柄。必须保存在 static 中保持存活，
/// 否则句柄被 drop 后播放会立即停止。
static AUDIO_SINK: LazyLock<Option<rodio::MixerDeviceSink>> =
    LazyLock::new(|| match rodio::DeviceSinkBuilder::open_default_sink() {
        Ok(sink) => Some(sink),
        Err(e) => {
            eprintln!("音频设备初始化失败，将静默运行: {e}");
            None
        }
    });

/// 播放一个音效。`name` 对应 `ui/assets` 下的音频文件名。
/// 音频文件在编译期嵌入可执行文件，运行时无需关心工作目录。
fn play_sound(name: &str) {
    let Some(sink) = AUDIO_SINK.as_ref() else {
        return; // 音频设备初始化失败，直接忽略
    };

    let bytes: &'static [u8] = match name {
        "爆炸" => include_bytes!("../ui/assets/爆炸.mp3"),
        "怪兽说话" => include_bytes!("../ui/assets/怪兽说话.mp3"),
        "bgm" => include_bytes!("../ui/assets/bgm.mp3"),
        _ => {
            eprintln!("未知音效: {name}");
            return;
        }
    };

    let source = match rodio::Decoder::try_from(Cursor::new(bytes)) {
        Ok(source) => source,
        Err(e) => {
            eprintln!("解码音效「{name}」失败: {e}");
            return;
        }
    };

    // BGM 循环播放，其他音效只播放一次。
    // 多个音效同时播放时会被 rodio 自动混音。
    if name == "bgm" {
        sink.mixer().add(source.repeat_infinite());
    } else {
        sink.mixer().add(source);
    }
}

/// 把字符串转成以 `\0` 结尾的 UTF-16 缓冲区（Win32 API 用）。
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 用户可见的消息框。release 构建没有控制台（windows_subsystem = "windows"），
/// print/eprintln 的输出会被静默丢弃，所以面向用户的提示必须走消息框。
fn show_message(title: &str, message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW};
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide(message).as_ptr(),
            wide(title).as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// 无参数直接运行时显示用法，并提供注册/卸载右键菜单的入口。
/// `auto_registered` 为 true 表示本次启动刚执行过自动注册。
fn show_help(exe: &Path, auto_registered: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IDNO, IDYES, MB_ICONQUESTION, MB_YESNOCANCEL, MessageBoxW,
    };

    let status = if auto_registered {
        "已自动注册"
    } else if context_menu::is_registered(exe) {
        "已注册"
    } else {
        "未注册"
    };
    let text = format!(
        "Monster Deleter（怪兽回收站）\n\n\
         用法：\n\
         \tmonster-deleter.exe <文件或文件夹路径>\n\
         \t将目标移入回收站（不会永久删除）。\n\
         \t从资源管理器右键菜单调用时无需手动指定路径。
         \t可随时按ESC键退出。\n\n\
         右键菜单状态：{status}\n\n\
         是(Y)：注册/更新右键菜单\n\
         否(N)：卸载右键菜单\n\
         取消：退出"
    );
    let choice = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide(&text).as_ptr(),
            wide("Monster Deleter").as_ptr(),
            MB_YESNOCANCEL | MB_ICONQUESTION,
        )
    };
    match choice {
        IDYES => match context_menu::register(exe) {
            Ok(()) => show_message("Monster Deleter", "右键菜单已注册。"),
            Err(e) => show_message("Monster Deleter", &format!("右键菜单注册失败：{e}")),
        },
        IDNO => match context_menu::unregister() {
            Ok(()) => show_message("Monster Deleter", "右键菜单已卸载。"),
            Err(e) => show_message("Monster Deleter", &format!("右键菜单卸载失败：{e}")),
        },
        _ => {}
    }
}

fn main() -> Result<(), slint::PlatformError> {
    // 隐藏模式：由主程序（动画窗口）派生的辅助进程，独立完成回收站操作后自行退出。
    // 大文件夹的回收站操作可能耗时数分钟（IFileOperation 同步执行），若让动画进程
    // 等待它，动画播完后台会残留一个卡在系统调用里、难以结束的进程。
    // 用法：monster-deleter.exe --delete <路径>
    let mut args = std::env::args_os();
    args.next(); // 程序自身路径
    if args.next().as_deref() == Some(std::ffi::OsStr::new("--delete")) {
        let Some(path) = args.next() else {
            return Ok(());
        };
        if let Err(e) = trash::send_to_trash(Path::new(&path)) {
            show_message("Monster Deleter", &e);
        }
        return Ok(());
    }

    // Windows 上 femtovg(OpenGL) 渲染器不支持窗口透明（透明区域会显示为黑色），
    // 改用软件渲染器以支持透明背景。
    slint::BackendSelector::new()
        .renderer_name("software".into())
        .select()?;

    // 提前打开音频设备，避免首次播放音效时卡顿
    LazyLock::force(&AUDIO_SINK);

    // 右键菜单：每次启动时检查，未注册（或指向的 exe 已变化）则自动注册。
    // 本次启动若执行了自动注册，帮助框会显示“已自动注册”，避免用户卸载后
    // 再次运行却看到“已注册”的困惑。
    let exe = std::env::current_exe().unwrap_or_default();
    let mut auto_registered = false;
    if !exe.as_os_str().is_empty() && !context_menu::is_registered(&exe) {
        match context_menu::register(&exe) {
            Ok(()) => auto_registered = true,
            Err(e) => show_message("Monster Deleter", &format!("右键菜单注册失败：{e}")),
        }
    }

    // 无参数：直接运行，显示用法与注册/卸载入口
    let Some(target) = std::env::args_os().nth(1) else {
        show_help(&exe, auto_registered);
        return Ok(());
    };
    let target = PathBuf::from(target);

    if !target.exists() {
        show_message("Monster Deleter", &format!("{} 不存在", target.display()));
        return Ok(());
    }

    // 启动时循环播放背景音乐（不需要的话删掉这一行即可）
    play_sound("bgm");

    let main_window = AppWindow::new()?;
    // 设置 quit 回调
    main_window.on_quit(|| {
        println!("Request to quit received.");
        slint::quit_event_loop().unwrap(); // 退出事件循环，关闭应用
    });

    main_window.on_play_sound(|sound| {
        play_sound(sound.as_str());
    });

    // 踹文件动画播放到指定帧时，把目标移入回收站。
    // 删除委托给隐藏的辅助进程（--delete 模式）执行：主进程动画播完立即退出，
    // 不等待删除结果；辅助进程独立完成删除后自行退出（是个普通进程，
    // 任务管理器中可见、可结束，不会把动画进程拖成“幽灵”）。
    let exe = std::env::current_exe().unwrap_or_default();
    let target = target.clone();
    main_window.on_send2trash(move || {
        if !exe.as_os_str().is_empty() {
            let _ = std::process::Command::new(&exe)
                .arg("--delete")
                .arg(&target)
                .spawn();
        }
    });

    main_window.run()
}
