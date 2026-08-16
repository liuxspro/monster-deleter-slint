use std::path::Path;

/// 将文件或文件夹移入回收站（不是永久删除）。
///
/// 右键菜单（Explorer）传入的是绝对路径；开发时可能传相对路径，
/// 所以先 canonicalize 转成绝对路径再交给系统处理。
pub fn send_to_trash(path: &Path) -> Result<(), String> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("无法解析路径 {}：{e}", path.display()))?;
    trash::delete(&path).map_err(|e| format!("无法删除 {}：{e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_to_trash_works() {
        let file = std::env::temp_dir().join(format!("trash_test_{}.txt", std::process::id()));
        std::fs::write(&file, b"test").unwrap();
        assert!(file.exists());

        send_to_trash(&file).unwrap();
        assert!(!file.exists());
    }
}
