#![no_std]
#![no_main]
// 不使用预定义的入口点，添加#![no_main]属性
#[macro_use]
extern crate user_lib;

#[no_mangle]
fn main() {
    println!("ApolloOS");
}