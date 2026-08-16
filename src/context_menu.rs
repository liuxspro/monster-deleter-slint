use std::io;
use std::path::Path;

use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

/// 注册表根路径模板。
///
/// - `*`：所有文件
/// - `Directory`：所有文件夹
/// - `lnkfile`：快捷方式。没有它的话，右键快捷方式时 shell 会把 .lnk
///   解析后的**目标文件**传给 `%1`，导致删掉的是目标而不是快捷方式本身。
const ROOTS: [&str; 3] = ["*", "Directory", "lnkfile"];

/// 注册表键名（与 Python 版 python/context.py 保持一致）。
const MENU_KEY: &str = "monster-deleter-slint";
const DISPLAY_NAME: &str = "召唤大将怪兽摧毁";

/// 注册到 command 子键的命令行，`%1` 由资源管理器替换为被操作的文件路径。
fn command_line(exe: &Path) -> String {
    format!("\"{}\" \"%1\"", exe.display())
}

fn icon_value(exe: &Path) -> String {
    format!("\"{}\",0", exe.display())
}

fn notify_shell() {
    use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

fn key_path(root: &str) -> String {
    format!(r"Software\Classes\{root}\shell\{MENU_KEY}")
}

/// 右键菜单是否已注册，且 command 指向当前 exe。
pub fn is_registered(exe: &Path) -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let expected = command_line(exe);
    for root in ROOTS {
        let Ok(key) = hkcu.open_subkey(format!(r"{}\command", key_path(root))) else {
            return false;
        };
        match key.get_value::<String, _>("") {
            Ok(value) if value == expected => {}
            _ => return false,
        }
    }
    true
}

/// 注册右键菜单（写入 HKCU，无需管理员权限）。
pub fn register(exe: &Path) -> io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let cmd = command_line(exe);
    for root in ROOTS {
        let (shell, _) = hkcu.create_subkey(key_path(root))?;
        shell.set_value("", &DISPLAY_NAME)?;
        shell.set_value("Icon", &icon_value(exe))?;
        let (command, _) = shell.create_subkey("command")?;
        command.set_value("", &cmd)?;
    }
    notify_shell();
    Ok(())
}

/// 卸载右键菜单。键不存在时忽略错误。
pub fn unregister() -> io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for root in ROOTS {
        let _ = hkcu.delete_subkey_all(key_path(root));
    }
    notify_shell();
    Ok(())
}
