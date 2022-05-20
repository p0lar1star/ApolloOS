use core::cell::{RefCell, RefMut};
// Cell和RefCell用于单线程的共享引用，很多时候，都是用在struct的field。
// 这样，你就可以共享这个struct，但是仍能够对某个field做修改。
pub struct UPSafeCell<T> {
    /// inner data
    inner: RefCell<T>,
}

unsafe impl<T> Sync for UPSafeCell<T> {}

impl<T> UPSafeCell<T> {
    /// User is responsible to guarantee that struct is only used in
    /// uniprocessor
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }
    /// Panic if the data has been borrowed
    pub fn exclusive_access(&self) -> RefMut<'_, T> {
        self.inner.borrow_mut()
    }
}
