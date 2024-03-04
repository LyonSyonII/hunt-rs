#![warn(missing_docs)]

//! The Bumpalo Herd
//!
//! From [https://github.com/vorner/bumpalo-herd]
//!
//! The [`bumpalo`] library let's one use a bump allocator, an interesting and fast strategy to
//! allocate a lot of small objects. Additionally, it helps solve some of Rust lifetime issues.
//!
//! Nevertheless, it is not [`Sync`], which makes it hard to use in many situations ‒ like in
//! [`rayon`] iterators or scoped threads.
//!
//! This library extends [`bumpalo`] with the [`Herd`] type. It represents a group of the [`Bump`]
//! allocators. Each thread then can get its own instance to allocate from. Unlike just creating
//! one for each thread the convenient way, the allocated memory can survive past the
//! thread/iterator termination, because the lifetime is tied to the [`Herd`] itself (the [`Bump`]
//! is rescued from the thread behind the scenes).
//!
//! # Examples
//!
//! We use the bump allocation inside a [`rayon`] iterator. The `map_init` allows us to grab an
//! arena and use it in each invocation cheaply, but unlike `Bump::new` allocated one, the memory
//! is usable after the iterator run to its termination.
//!
//! ```rust
//! # use bumpalo_herd::Herd;
//! # use rayon::prelude::*;
//!
//! // Bunch of Bump instances that can be borrowed
//! let mut herd = Herd::new();
//!
//! let ints: Vec<&mut usize> = (0usize..1_000)
//!     .into_par_iter()
//!     .map_init(|| herd.get(), |bump, i| {
//!         // We have an allocator here we can play with.
//!         // The computation would be something bigger, sure.
//!         bump.alloc(i)
//!     })
//!     .collect();
//!
//! // Available here even though the iterator has already ended.
//! dbg!(ints);
//!
//! // Deallocate the memory
//! herd.reset();
//!
//! // Won't work any more, memory gone
//! // (don't worry, Rust won't let us)
//! // dbg!(ints);
//! ```
//!
//! Similar thing can be done with [scoped threads] from [`crossbeam_utils`]. If we got our
//! allocator through `Bump::new`, we wouldn't be allowed to use the results in the other thread.
//!
//! ```rust
//! # use std::sync::mpsc;
//! # use bumpalo_herd::Herd;
//! # use crossbeam_utils::thread;
//!
//! let herd = Herd::new();
//! let (sender, receiver) = mpsc::sync_channel(10);
//!
//! // Scoped threads from crossbeam_utils.
//! // Producer and consumer, can send data from one to another.
//! // The data lives long enough.
//! thread::scope(|s| {
//!     s.spawn(|_| {
//!         let bump = herd.get();
//!         let s = bump.alloc_str("Hello");
//!         sender.send(s).unwrap();
//!         drop(sender); // Close the channel.
//!     });
//!     s.spawn(|_| {
//!         for s in receiver {
//!             dbg!(s);
//!         }
//!     });
//! }).unwrap();
//! ```
//!
//! [`rayon`]: https://docs.rs/rayon
//! [`crossbeam_utils`]: https://docs.rs/crossbeam_utils
//! [scoped threads]: https://docs.rs/crossbeam-utils/0.7.*/crossbeam_utils/thread/index.html

use std::alloc::Layout;
use std::mem::ManuallyDrop;
use std::ptr::NonNull;
use parking_lot::Mutex;

use bumpalo::Bump;

type HerdInner = Vec<Box<Bump>>;

/// A group of [`Bump`] allocators.
///
/// This contains a bunch of [`Bump`] allocators. They can be borrowed into threads with
/// [`get`][Herd::get]. Once the returned [`Member`] proxies get dropped, they return back in here.
/// This means they can be used again, by more threads. But more importantly, the memory allocated
/// from them is still valid.
///
/// The allocators are created on demand ‒ if no existing ones are cached inside, new one is
/// created.
#[derive(Default, Debug)]
pub struct Herd(Mutex<HerdInner>);

impl Herd {
    /// Creates a new [`Herd`].
    ///
    /// No allocators are created at that point, it is empty, but will be populated on use.
    pub fn new() -> Self {
        Self::default()
    }

    /// Deallocates all memory from all the allocators.
    ///
    /// This is similar to [`Bump::reset`] from [`bumpalo`], but works on all the allocators of
    /// this herd. Note that this takes `&mut`, they can't be in active use by any threads at this
    /// point.
    ///
    /// Note that it is not possible to reset individual [`Bump`]s as the memory in there could
    /// belong to some other (previous) thread and the lifetime of allocated objects are not tied
    /// to them, only to the [`Herd`].
    pub fn reset(&mut self) {
        for e in self.0.get_mut().iter_mut() {
            e.reset();
        }
    }

    /// Borrows a member allocator from this herd.
    ///
    /// As the [`Herd`] is [`Sync`], it is possible to call this from the worker threads. The
    /// [`Member`] is a proxy around [`Bump`], allowing to allocate objects with lifetime of the
    /// [`Herd`] (therefore, the allocated objects can live longer than the [`Member`] itself).
    ///
    /// # Performance note
    ///
    /// This is not cheap and is not expected to happen often. It contains a mutex.
    ///
    /// The expected usage pattern is that each worker thread (or similar entity) grabs one
    /// allocator *once*, at its start and uses it through its lifetime, not that it would call
    /// `get` on each allocation.
    pub fn get(&self) -> Member<'_> {
        let mut lock = self.0.lock();
        let bump = lock.pop().unwrap_or_default();
        Member {
            arena: ManuallyDrop::new(bump),
            owner: self,
        }
    }
}

/// A proxy for a [`Bump`].
///
/// You get one by [`Herd::get`].
///
/// The purpose is twofold:
///
/// * To return the inner [`Bump`] back to its [`Herd`] once this proxy is dropped.
/// * To allow allocation of objects with lifetime tied to the [`Herd`].
///
/// # Note
///
/// * Not all the allocation methods are exposed right now. If the others are needed, you're
///   welcome to send a PR.
/// * The allocation methods are not documented here. They however correspond 1:1 to the same-named
///   methods on [`Bump`]. See their documentation.
#[derive(Debug)]
pub struct Member<'h> {
    arena: ManuallyDrop<Box<Bump>>,
    owner: &'h Herd,
}

macro_rules! alloc_fn {
    ($(pub fn $name: ident<($($g: tt)*)>(&self, $($pname: ident: $pty: ty),*) -> $res: ty;)*) => {
        $(
            pub fn $name<$($g)*>(&self, $($pname: $pty),*) -> $res {
                self.extend(self.arena.$name($($pname),*))
            }
        )*
    }
}

#[allow(missing_docs)] // Macro-generated; same as the ones on Bump
impl<'h> Member<'h> {
    alloc_fn! {
        pub fn alloc<(T)>(&self, val: T) -> &'h mut T;
        pub fn alloc_with<(T, F: FnOnce() -> T)>(&self, f: F) -> &'h mut T;
        pub fn alloc_str<()>(&self, src: &str) -> &'h mut str;
        pub fn alloc_slice_clone<(T: Clone)>(&self, src: &[T]) -> &'h mut [T];
        pub fn alloc_slice_copy<(T: Copy)>(&self, src: &[T]) -> &'h mut [T];
        pub fn alloc_slice_fill_clone<(T: Clone)>(&self, len: usize, value: &T) -> &'h mut [T];
        pub fn alloc_slice_fill_copy<(T: Copy)>(&self, len: usize, value: T) -> &'h mut [T];
        pub fn alloc_slice_fill_default<(T: Default)>(&self, len: usize) -> &'h mut [T];
        pub fn alloc_slice_fill_with<(T, F: FnMut(usize) -> T)>(&self, len: usize, f: F)
            -> &'h mut [T];
    }

    pub fn alloc_slice_fill_iter<T, I>(&self, iter: I) -> &'h mut [T]
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: ExactSizeIterator,
    {
        self.extend(self.arena.alloc_slice_fill_iter(iter))
    }
    
    pub fn alloc_layout(&self, layout: Layout) -> NonNull<u8> {
        self.arena.as_ref().alloc_layout(layout)
    }
}

impl<'h> Member<'h> {
    /*
     * We are extending the lifetime past what Rust believes is right. This is OK, because while
     * the Member can be dropped and we move the Box, the Bump inside stays at the same place.
     * Therefore, it doesn't even "notice" in practice that anything happened and the Bump actually
     * does live for 'h.
     *
     * To ensure that, we:
     * * Move it back into the herd on drop.
     * * If it is not dropped, it has to be *leaked* (therefore stays at one place in memory
     *   forever), which is bad, but doesn't cause UB.
     * * It can't be taken out (we don't provide any &mut or anything).
     */
    fn extend<'s, T: ?Sized>(&'s self, v: &'s mut T) -> &'h mut T {
        let result = v as *mut T;
        unsafe { &mut *result }
    }
    
    // Note: This *can't* return `&'h Bump`. That way one could keep a reference, drop the Member
    // and let another thread take it - that would allow both to allocate from the same Bump which
    // would be UB.
    //
    // It also can't return anything like &mut Bump, because then people could reset memory they
    // don't own (from previous borrow of the Bump).
    /// Access the [`Bump`] inside.
    ///
    /// This can be used to get the [`Bump`] allocator itself, if something needs the specific
    /// type. Note that the lifetime of the [`Bump`] and the values allocated from these is tied to
    /// *this* [`Member`], not to the [`Herd`].
    ///
    /// As this is going to be used rarely and potentially does something else then desired (eg.
    /// the shorter lifetime), this is an explicit method, not [`Deref`][std::ops::Deref].
    pub fn as_bump(&self) -> &Bump {
        &self.arena
    }
}

impl Drop for Member<'_> {
    fn drop(&mut self) {
        // If the unwrap panics, we will just leak, not destroy, the arena.
        let mut lock = self.owner.0.lock();
        /*
         * Safety considerations.
         *
         * The only requirement is that the self.arena is not ever used again. This is trivial, we
         * are in the destructor.
         *
         * We also need to ensure the member is not dropped in here (otherwise we would destroy
         * memory that's still lifetime-OK according to the `'h`. But push doesn't panic
         * (allocators are disallowed from panicking).
         */
        let member = unsafe { ManuallyDrop::take(&mut self.arena) };
        lock.push(member);
    }
}