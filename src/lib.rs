#![no_std]

//! The `array-vec` crate allows you to initialize arrays
//! with an initializer closure that will be called
//! once for each element until the array is filled.
//!
//! This way you do not need to default-fill an array
//! before running initializers. Rust currently only
//! lets you either specify all initializers at once,
//! individually (`[a(), b(), c(), ...]`), or specify
//! one initializer for a `Copy` type (`[a(); N]`),
//! which will be called once with the result copied over.
//!
//! # Examples:
//! ```rust
//! # #![allow(unused)]
//! # extern crate array_init;
//!
//! // Initialize an array of length 10 containing
//! // successive squares
//!
//! let arr: [u32; 50] = array_init::array_init(|i| (i*i) as u32);
//! 
//! // Initialize an array from an iterator
//! // producing an array of [1,2,3,4] repeated
//!
//! let four = [1u32,2,3,4];
//! let mut iter = four.iter().cloned().cycle();
//! let arr: [u32; 50] = array_init::from_iter(iter).unwrap();
//! 
//! // Closures can also mutate state. We guarantee that they will be called
//! // in order from lower to higher indices.
//! 
//! let mut last = 1u64;
//! let mut secondlast = 0;
//! let fibonacci: [u64; 50] = array_init::array_init(|_| {
//!     let this = last + secondlast;
//!     secondlast = last;
//!     last = this;
//!     this
//! });
//! ```
//! 
//! Currently, using `from_iter` and `array_init` will incur additional
//! memcpys, which may be undesirable for a large array. This can be eliminated
//! by using the nightly feature of this crate, which uses unions to provide
//! panic-safety. Alternatively, if your array only contains `Copy` types,
//! you can use `array_init_copy` and `from_iter_copy`.
//!
//! Sadly, cannot guarantee right now that any of these solutions will completely
//! eliminate a memcpy.
//!

extern crate nodrop;

use nodrop::NoDrop;
use core::mem;

/// Trait for things which are actually arrays
///
/// Probably shouldn't implement this yourself,
/// but you can
pub unsafe trait IsArray {
    type Item;
    /// Must assume self is uninitialized.
    fn set(&mut self, idx: usize, value: Self::Item);
    fn len() -> usize;
}

#[inline]
/// Initialize an array given an initializer expression
///
/// The initializer is given the index of the element. It is allowed
/// to mutate external state; we will always initialize the elements in order.
///
/// Without the nightly feature it is very likely that this will cause memcpys.
/// For panic safety, we internally use NoDrop, which will ensure that panics
/// in the initializer will not cause the array to be prematurely dropped.
/// If you are using a Copy type, prefer using `array_init_copy` since
/// it does not need the panic safety stuff and is more likely to have no
/// memcpys.
///
/// # Examples
///
/// ```rust
/// # #![allow(unused)]
/// # extern crate array_init;
///
/// // Initialize an array of length 10 containing
/// // successive squares
///
/// let arr: [u32; 50] = array_init::array_init(|i| (i*i) as u32);
/// 
/// // Initialize an array from an iterator
/// // producing an array of [1,2,3,4] repeated
///
/// let four = [1u32,2,3,4];
/// let mut iter = four.iter().cloned().cycle();
/// let arr: [u32; 50] = array_init::from_iter(iter).unwrap();
/// 
/// ```
///
pub fn array_init<Array, F>(mut initializer: F) -> Array where Array: IsArray,
                                                               F: FnMut(usize) -> Array::Item {
    // NoDrop makes this panic-safe
    // We are sure to initialize the whole array here,
    // and we do not read from the array till then, so this is safe.
    let mut ret: NoDrop<Array> = NoDrop::new(unsafe { mem::uninitialized() });
    for i in 0..Array::len() {
        Array::set(&mut ret, i, initializer(i));
    }
    ret.into_inner()
}

#[inline]
/// Initialize an array given an iterator
///
/// We will iterate until the array is full or the iterator is exhausted. Returns
/// None if the iterator is exhausted before we can fill the array.
///
/// Without the nightly feature it is very likely that this will cause memcpys.
/// For panic safety, we internally use NoDrop, which will ensure that panics
/// in the initializer will not cause the array to be prematurely dropped.
/// If you are using a Copy type, prefer using `from_iter_copy` since
/// it does not need the panic safety stuff and is more likely to have no
/// memcpys.
///
/// # Examples
///
/// ```rust
/// # #![allow(unused)]
/// # extern crate array_init;
///
/// // Initialize an array from an iterator
/// // producing an array of [1,2,3,4] repeated
///
/// let four = [1u32,2,3,4];
/// let mut iter = four.iter().cloned().cycle();
/// let arr: [u32; 50] = array_init::from_iter_copy(iter).unwrap();
/// ```
///
pub fn from_iter<Array, I>(iter: I) -> Option<Array>
    where I: IntoIterator<Item = Array::Item>,
          Array: IsArray {
    // NoDrop makes this panic-safe
    // We are sure to initialize the whole array here,
    // and we do not read from the array till then, so this is safe.
    let mut ret: NoDrop<Array> = NoDrop::new(unsafe { mem::uninitialized() });
    let mut count = 0;
    for item in iter.into_iter().take(Array::len()) {
        Array::set(&mut ret, count, item);
        count += 1;
    }
    // crucial for safety!
    if count == Array::len() {
        Some(ret.into_inner())
    } else {
        None
    }
}

#[inline]
/// Initialize an array of `Copy` elements given an initializer expression
///
/// The initializer is given the index of the element. It is allowed
/// to mutate external state; we will always initialize the elements in order.
///
/// This is preferred over `array_init` if you have a `Copy` type
///
/// # Examples
///
/// ```rust
/// # #![allow(unused)]
/// # extern crate array_init;
///
/// // Initialize an array of length 10 containing
/// // successive squares
///
/// let arr: [u32; 50] = array_init::array_init_copy(|i| (i*i) as u32);
/// 
/// 
/// // Closures can also mutate state. We guarantee that they will be called
/// // in order from lower to higher indices.
/// 
/// let mut last = 1u64;
/// let mut secondlast = 0;
/// let fibonacci: [u64; 50] = array_init::array_init_copy(|_| {
///     let this = last + secondlast;
///     secondlast = last;
///     last = this;
///     this
/// });
/// ```
///
pub fn array_init_copy<Array, F>(mut initializer: F) -> Array where Array: IsArray,
                                                                    F: FnMut(usize) -> Array::Item,
                                                                    Array::Item : Copy {
    // We are sure to initialize the whole array here,
    // and we do not read from the array till then, so this is safe.
    let mut ret: Array = unsafe { mem::uninitialized() };
    for i in 0..Array::len() {
        Array::set(&mut ret, i, initializer(i));
    }
    ret
}

#[inline]
/// Initialize an array given an iterator
///
/// We will iterate until the array is full or the iterator is exhausted. Returns
/// None if the iterator is exhausted before we can fill the array.
///
/// This is preferred over `from_iter_copy` if you have a `Copy` type
///
/// # Examples
///
/// ```rust
/// # #![allow(unused)]
/// # extern crate array_init;
///
/// // Initialize an array from an iterator
/// // producing an array of [1,2,3,4] repeated
///
/// let four = [1u32,2,3,4];
/// let mut iter = four.iter().cloned().cycle();
/// let arr: [u32; 50] = array_init::from_iter_copy(iter).unwrap();
/// ```
pub fn from_iter_copy<Array, I>(iter: I) -> Option<Array>
    where I: IntoIterator<Item = Array::Item>,
          Array: IsArray,
          Array::Item : Copy {
    // We are sure to initialize the whole array here,
    // and we do not read from the array till then, so this is safe.
    let mut ret: Array = unsafe { mem::uninitialized() };
    let mut count = 0;
    for item in iter.into_iter().take(Array::len()) {
        Array::set(&mut ret, count, item);
        count += 1;
    }
    // crucial for safety!
    if count == Array::len() {
        Some(ret)
    } else {
        None
    }
}

macro_rules! impl_is_array {
    ($size:expr) => (
        unsafe impl<T> IsArray for [T; $size] {
            type Item = T;
            #[inline]
            fn set(&mut self, idx: usize, value: Self::Item) {
                mem::forget(mem::replace(&mut self[idx], value));
            }

            #[inline]
            fn len() -> usize {
                $size
            }
        }
    )
}

// lol

impl_is_array!(0);
impl_is_array!(1);
impl_is_array!(2);
impl_is_array!(3);
impl_is_array!(4);
impl_is_array!(5);
impl_is_array!(6);
impl_is_array!(7);
impl_is_array!(8);
impl_is_array!(9);
impl_is_array!(10);
impl_is_array!(11);
impl_is_array!(12);
impl_is_array!(13);
impl_is_array!(14);
impl_is_array!(15);
impl_is_array!(16);
impl_is_array!(17);
impl_is_array!(18);
impl_is_array!(19);
impl_is_array!(20);
impl_is_array!(21);
impl_is_array!(22);
impl_is_array!(23);
impl_is_array!(24);
impl_is_array!(25);
impl_is_array!(26);
impl_is_array!(27);
impl_is_array!(28);
impl_is_array!(29);
impl_is_array!(30);
impl_is_array!(31);
impl_is_array!(32);
impl_is_array!(33);
impl_is_array!(34);
impl_is_array!(35);
impl_is_array!(36);
impl_is_array!(37);
impl_is_array!(38);
impl_is_array!(39);
impl_is_array!(40);
impl_is_array!(41);
impl_is_array!(42);
impl_is_array!(43);
impl_is_array!(44);
impl_is_array!(45);
impl_is_array!(46);
impl_is_array!(47);
impl_is_array!(48);
impl_is_array!(49);
impl_is_array!(50);
impl_is_array!(51);
impl_is_array!(52);
impl_is_array!(53);
impl_is_array!(54);
impl_is_array!(55);
impl_is_array!(56);
impl_is_array!(57);
impl_is_array!(58);
impl_is_array!(59);
impl_is_array!(60);
impl_is_array!(61);
impl_is_array!(62);
impl_is_array!(63);
impl_is_array!(64);
