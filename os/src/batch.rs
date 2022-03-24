// os/src/batch.rs
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use core::arch::asm;
use lazy_static::*;

const USER_STACK_SIZE: usize = 4096 * 2;
const KERNEL_STACK_SIZE: usize = 4096 * 2;
const MAX_APP_NUM: usize = 16;
const APP_BASE_ADDRESS: usize = 0x80400000;
const APP_SIZE_LIMIT: usize = 0x20000;

struct AppManager {
    num_app: usize,
    current_app: usize,
    app_start: [usize; MAX_APP_NUM + 1],
}

lazt_static! {
    static ref APP_MANAGER: UPSsfeCell<AppManager> = unsafe {
        UPSafeCell::new({
            extern "C" {
                fn _num_app();// 找到 link_app.S 中提供的符号 _num_app
            }
            let num_app_ptr = _num_app as usize as *const usize;
            let num_app = num_app_ptr.read_volatile();//从这里开始解析出应用数量以及各个应用的起始地址
            let mut app_start: [usize, MAX_APP_NUM + 1] = [0; MAX_APP_NUM + 1];
            // 从这里开始解析出应用数量以及各个应用的起始地址
            let app_start_raw: &[usize] = core::slice::from_raw_parts(
                num_app_ptr.add(1), num_app + 1
            );
            app_start[.. = num_app].copy_from_slice(app_start_raw);
            AppManager {
                num_app,
                current_app: 0,
                app_start,
            }
        })
    };
}

impl AppManager {
    // print the number of loaded apps
    // and start&end addr of loaded apps
    pub fn print_app_info(&self) {
        println!("[kernel] num_app = {}", self.app_num);
        for i in 0..self.num_app {
            // print start_addr and end_addr
            println!(
                "[kernel] app_{} [{:#x}, {:#x})",
                i,
                self.app_start[i],
                self.app_start[i + 1]
            );
        }
    }

    pub fn get_cureent_app(&self) -> usize {
        self.current_app
    }

    pub fn move_to_next_app(&mut self) {
        self.current_app += 1;
    }

    // 负责将参数 app_id 对应的应用程序的二进制镜像
    // 加载到物理内存以 0x80400000 起始的位置
    unsafe fn load_app(&self, app_id: usize) {
        if app_id >= self.num_app {
            panic!("All applications completed!");
        }
        println!("[kernel] Loading app_{}", app_id);
        // clear icache
        asm("fence.i");
        // clear app area
        core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, APP_SIZE_LIMIT).fill(0);
        // app_src is an unmutable slice from app_start[id] location, length is the size of app
        let app_src = core::slice::from_raw_parts(
            self.app_start[app_id] as *const u8,
            self.app_start[app_id + 1] - self.app_start[app_id],
        );
        // app_dst is a mutable slice from 0x80400000, length is the size of app
        let app_dst = core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, app_src.len());
        // load to memory
        app_dst.copy_from_slice(app_src);
    }
}

pub fn init() {
    print_app_info();
}

pub fn print_app_info() {
    APP_MANAGER.exclusive_access().print_app_info();
}

pub fn run_next_app() {
    
}