// Windows release 禁止额外控制台窗口；勿删本属性。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    lanjing_lib::run();
}
