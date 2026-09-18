// 隐藏 release 构建下的控制台窗口（仅 Windows 生效）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tiny_reader_lib::run()
}
