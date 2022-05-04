/// 获得连接到内核数据段上的应用数目，
/// 在本节中，应用仍然是通过link_app.S链接到内核的数据段中的
pub fn get_num_app() -> usize {
    extern "C" {
        fn _num_app();
    }
    unsafe { (_num_app as usize as *const usize).read_volatile() }
}

/// 根据传入的应用编号取出对应应用的ELF格式的可执行文件数据
/// 在本节中，应用仍然是通过link_app.S链接到内核的数据段中的
pub fn get_app_data(app_id: usize) -> &'static [u8] {
    extern "C" {
        fn _num_app();
    }
    // 裸指针
    let num_app_ptr = _num_app as usize as *const usize;
    let num_app = get_num_app();
    // 读取各应用在内核数据段上的起始地址
    // 保存在app_start数组中
    let app_start = unsafe { core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1) };
    assert!(app_id < num_app);
    unsafe {
        core::slice::from_raw_parts(
            app_start[app_id] as *const u8,
            app_start[app_id + 1] - app_start[app_id],
        )
    }
}
