use core::alloc::Layout;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use std::{
    cell::{SyncUnsafeCell, UnsafeCell},
    fmt::Debug,
};

use allocator_api2::alloc::{AllocError, Allocator};
use refcell_lock_api::raw::CellMutex;
use talc::{ClaimOnOom, ErrOnOom, Span, Talc, Talck};

// const STACK_ALLOCATION: usize = 24 * 1024;

#[global_allocator]
static ALLOCATOR: Talck<spin::Mutex<()>, ClaimOnOom> = Talc::new(unsafe {
    static HEAP: SyncUnsafeCell<[u8; 512 * 1024]> = SyncUnsafeCell::new([0u8; 512 * 1024]);
    ClaimOnOom::new(Span::from_array(HEAP.get()))
})
.lock();

// Why `StackAllocator`  shouldn't own `Talck` without lifetime:
// ```
//  let allocator: StackAllocator = todo!();
//  let vec = allocator_api2::vec::Vec::<u8, StackAllocator>::new_in(allocator);
//  let leaked_buf = vec.leak::<'static>();
//  drop(allocator);
//  dbg!(leaked_buf); // dangling pointer here!
// ```
#[derive(Clone, Copy)]
pub struct StackAllocator<'a>(&'a Talck<spin::Mutex<()>, ClaimOnOom>);

impl<'a> Debug for StackAllocator<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("StackAllocator").finish()
    }
}

unsafe impl<'a> Allocator for StackAllocator<'a> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.0.allocate(layout)
    }
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        unsafe { self.0.deallocate(ptr, layout) }
    }
    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { self.0.allocate_zeroed(layout) }
    }
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { self.0.grow(ptr, old_layout, new_layout) }
    }
    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { self.0.grow_zeroed(ptr, old_layout, new_layout) }
    }
    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { self.0.shrink(ptr, old_layout, new_layout) }
    }
    fn by_ref(&self) -> &Self
    where
        Self: Sized,
    {
        self
    }
}

// TODO: remove StackAllocator (golang only allocate 32k for signal handlers stacks) 
// and directly use global spin allocator. 
pub fn with_stack_allocator<R, F: FnOnce(StackAllocator<'_>) -> R>(f: F) -> R {
    // let mut memory: MaybeUninit<[u8; STACK_ALLOCATION]> = MaybeUninit::uninit();
    // let talck = Talc::new(ErrOnOom).lock::<CellMutex>();
    // unsafe {
    //     talck
    //         .lock()
    //         .claim(talc::Span::from_array(memory.as_mut_ptr()));
    // };
    f(StackAllocator(&ALLOCATOR))
}
