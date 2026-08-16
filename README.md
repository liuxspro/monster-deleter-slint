# Monster Deleter（Slint 版）

学习 [Slint](https://slint.dev) 编程的练习项目

用 **Rust + Slint** 复刻了「怪兽回收站」小工具——选中文件/文件夹后，召唤大将怪兽把它消灭。

项目以 [MonsterDeleter](https://github.com/531149627/MonsterDeleter)（Python+PyQt6 版）为蓝本，动画与音效资源取自原项目，界面则完全用 Slint 重新实现。

## 学习要点

- **声明式 UI**：`.slint` 组件拆分（`ui/components/`）、属性绑定、回调（callback）与动画过渡
- **精灵动画**：基于 sprite sheet 实现逐帧动画（走路、踹文件、爆炸、登场、飞走）
- **Rust ↔ UI 交互**：UI 回调驱动 Rust 侧逻辑（移入回收站、播放音效），`include_modules!` 生成绑定代码
- **工程细节**：图片/音效编译期嵌入、透明无边框窗口（软件渲染）、右键菜单注册（HKCU）、release 体积优化

## 功能

- 资源管理器右键菜单集成「召唤大将怪兽摧毁」，一键调用
- 目标移入回收站而非永久删除，按 `ESC` 随时退出
- 启动时自动检查并注册右键菜单（写入 HKCU，无需管理员权限）；无参数运行时提供注册/卸载入口

## 使用

```sh
# 构建（仅支持 Windows）
cargo build --release

# 无参数：显示帮助，注册/卸载右键菜单
target\release\monster-deleter.exe

# 指定路径：直接播放删除动画
target\release\monster-deleter.exe <文件或文件夹路径>
```

## 致谢

- [MonsterDeleter](https://github.com/531149627/MonsterDeleter)：动画与音效资源来源
