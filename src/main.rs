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
    // 删除放到后台线程执行：回收大文件夹可能耗时较长，若在 UI 线程同步执行，
    // 会冻结爆炸动画（程序是全屏覆盖，用户此时什么都做不了）。
    // 主线程在 run() 返回后通过 done_rx 等待删除完成，避免进程提前退出、
    // 把进行到一半的回收站操作掐断。
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let target = target.clone();
    main_window.on_send2trash(move || {
        let done_tx = done_tx.clone();
        let target = target.clone();
        std::thread::spawn(move || {
            if let Err(e) = trash::send_to_trash(&target) {
                show_message("Monster Deleter", &e);
            }
            let _ = done_tx.send(());
        });
    });

    main_window.run()?;

    // 释放窗口（其回调持有的 done_tx 随之 drop）。此后：
    // - 若删除线程已触发：recv 阻塞直到它报告完成，进程不会提前退出；
    // - 若从未触发删除（例如提前 ESC 退出）：发送端已全部 drop，recv 立即返回，不阻塞。
    drop(main_window);
    let _ = done_rx.recv();
    Ok(())
}
