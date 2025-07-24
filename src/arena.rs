
use std::cell::RefCell;

use crate::PathAccess;
use bumpalo::Bump;
use blink_alloc::SyncBlinkAlloc;
use allocator_api2::vec::Vec;
use thread_local::ThreadLocal;

#[ouroboros::self_referencing]
#[derive(Debug)]
pub struct PathAccessArena {
    pub bump: Bump,
    #[borrows(bump)]
    #[covariant]
    pub accesses: Vec<PathAccess<'this>, &'this Bump>,
}

impl Default for PathAccessArena {
    fn default() -> Self {
       Self::new(Bump::new(), |bump| {
            Vec::new_in(bump)
       })
    }
}

unsafe impl Send for PathAccessArena {
    
}

// impl PathAccessArena {
//     pub fn as_slice(&self) -> &[PathAccess<'_>] {
//         self.borrow_accesses().as_slice()
//     }
// }
