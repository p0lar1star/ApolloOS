// os/src/lang_item.rs
// Rust 的核心库core，可以理解为是经过大幅精简的标准库
// 它被应用在标准库不能覆盖到的某些特定领域，如裸机环境下
// 用于操作系统和嵌入式系统的开发，它不需要底层操作系统的支持。
use core::panic::PanicInfo;
#[panic_handler]
fn panic(panic_info: &PanicInfo) -> ! {
    let err = panic_info.message().unwrap();
    if let Some(location) = panic_info.location() {
        println!(
            "Panicked at {}:{} {}",
            location.file(),
            location.line(),
            err
        );
    } else {
        println!("Panicked: {}", err);
    }
    loop{}
}