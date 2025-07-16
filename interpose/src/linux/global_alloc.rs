use std::{alloc::{GlobalAlloc, System}, sync::atomic::{AtomicBool, Ordering}};

struct GlobalAllocator {
    enabled: AtomicBool
}

#[global_allocator]
static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator {
    enabled: AtomicBool::new(true)
};

impl GlobalAllocator {
    fn check_enabled(&self) {
        if !self.enabled.load(Ordering::Relaxed) {
            libc_print::libc_eprintln!("allocation detected in signal handlers");
            unsafe { libc::abort() }
        }
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        self.check_enabled();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, new_size: usize) -> *mut u8 {
        self.check_enabled();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        self.check_enabled();
         unsafe { System.alloc(layout) }
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
         self.check_enabled();
         unsafe { System.dealloc(ptr, layout) }
    }
}

pub fn disable_alloc() {
    GLOBAL_ALLOCATOR.enabled.store(false, Ordering::Relaxed);
}
