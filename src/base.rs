// Depending upon token option, maybe treat expression as a `Result` or `Option`, propogating
// "negatives" (Err, None) using `?` operator when appropriate. Allows user to write code just once
// which could be used in both a failing and non-failing context
macro_rules! prop_negative {
    ($stmt:expr, naked) => (
        $stmt
    );
    ($stmt:expr, result) => (
        $stmt?
    );
    ($stmt:expr, option) => (
        $stmt?
    );
}

// Depending upon token option, maybe treat expression as `Future`, awaiting upon said expression
// when appropriate. Allows user to write code just once which could be used in both a synchronous
// and asynchronous context
macro_rules! should_await {
    ($stmt:expr, synchronous) => (
        $stmt
    );
    ($stmt:expr, asynchronous) => (
        ($stmt).await
    );
}

// Depending upon token option, wrap expression in Ok, Some, or nothing when appropriate.
// Useful for returning an expression in a block in a macro which could be expecting Option<T>,
// Result<T>, or just T.
macro_rules! positive_variant {
    ($stmt:expr, naked) => (
        $stmt
    );
    ($stmt:expr, result) => (
        Ok($stmt)
    );
    ($stmt:expr, option) => (
        Some($stmt)
    );
}

// Parameters:
//   * initializer: function-like token tree which will be used to initialize each element.
//   * T: the type of each element
//   * N: size of array (const usize)
//   * D: direction (const 1 or -1). If 1, initialize forward, else initialize backwards
//   * residue: token option (naked, option, or result) which provides information about what wraps
//              the values yielded by the initializer. Determines propogation and further wrapping
//   * sync_mode: token option (synchronous or asynchronous) to await on initializer or not
// Returns:
//   A token tree, specifically an if/else branch which implements all array-init functionality
macro_rules! base_array_init_impl {
    ($initializer:tt, $T:ty, $N:expr, $D:expr, $residue:tt, $sync_mode:tt) => {
        // The implementation differentiates two cases:
        //   A) `T` does not need to be dropped. Even if the initializer panics
        //      or returns `Err` we will not leak memory.
        //   B) `T` needs to be dropped. We must keep track of which elements have
        //      been initialized so far, and drop them if we encounter a panic or `Err` midway.
        if !core::mem::needs_drop::<$T>() {
            let mut array: core::mem::MaybeUninit<[$T; $N]> = core::mem::MaybeUninit::uninit();
            // pointer to array = *mut [T; N] <-> *mut T = pointer to first element
            let mut ptr_i = array.as_mut_ptr() as *mut $T;

            // # Safety
            //
            //   - for D > 0, we are within the array since we start from the
            //     beginning of the array, and we have `0 <= i < N`.
            //   - for D < 0, we start at the end of the array and go back one
            //     place before writing, going back N times in total, finishing
            //     at the start of the array.
            unsafe {
                if $D < 0 {
                    ptr_i = ptr_i.add($N);
                }

                for i in 0..$N {
                    let value_i = prop_negative!(should_await!($initializer(i), $sync_mode), $residue);
                    // We overwrite *ptr_i previously undefined value without reading or dropping it.
                    if $D < 0 {
                        ptr_i = ptr_i.sub(1);
                    }
                    ptr_i.write(value_i);
                    if $D > 0 {
                        ptr_i = ptr_i.add(1);
                    }
                }

                positive_variant!(array.assume_init(), $residue)
            }
        } else {
            // else: `mem::needs_drop::<T>()`

            /// # Safety
            ///
            ///   - `base_ptr[.. initialized_count]` must be a slice of initialized elements...
            ///
            ///   - ... that must be sound to `ptr::drop_in_place` if/when
            ///     `UnsafeDropSliceGuard` is dropped: "symbolic ownership"
            struct UnsafeDropSliceGuard<Item> {
                base_ptr: *mut Item,
                initialized_count: usize,
            }

            impl<Item> Drop for UnsafeDropSliceGuard<Item> {
                fn drop(self: &'_ mut Self) {
                    unsafe {
                        // # Safety
                        //
                        //   - the contract of the struct guarantees that this is sound
                        core::ptr::drop_in_place(core::slice::from_raw_parts_mut(
                            self.base_ptr,
                            self.initialized_count,
                        ));
                    }
                }
            }

            //  If the `initializer(i)` call panics, `panic_guard` is dropped,
            //  dropping `array[.. initialized_count]` => no memory leak!
            //
            // # Safety
            //
            //  1. - For D > 0, by construction, array[.. initiliazed_count] only
            //       contains init elements, thus there is no risk of dropping
            //       uninit data;
            //     - For D < 0, by construction, array[N - initialized_count..] only
            //       contains init elements.
            //
            //  2. - for D > 0, we are within the array since we start from the
            //       beginning of the array, and we have `0 <= i < N`.
            //     - for D < 0, we start at the end of the array and go back one
            //       place before writing, going back N times in total, finishing
            //       at the start of the array.
            //
            unsafe {
                let mut array: core::mem::MaybeUninit<[$T; $N]> = core::mem::MaybeUninit::uninit();
                // pointer to array = *mut [T; N] <-> *mut T = pointer to first element
                let mut ptr_i = array.as_mut_ptr() as *mut $T;
                if $D < 0 {
                    ptr_i = ptr_i.add(N);
                }
                let mut panic_guard = UnsafeDropSliceGuard {
                    base_ptr: ptr_i,
                    initialized_count: 0,
                };

                for i in 0..$N {
                    // Invariant: `i` elements have already been initialized
                    panic_guard.initialized_count = i;
                    // If this panics or fails, `panic_guard` is dropped, thus
                    // dropping the elements in `base_ptr[.. i]` for D > 0 or
                    // `base_ptr[N - i..]` for D < 0.
                    let value_i = prop_negative!(should_await!($initializer(i), $sync_mode), $residue);
                    // this cannot panic
                    // the previously uninit value is overwritten without being read or dropped
                    if $D < 0 {
                        ptr_i = ptr_i.sub(1);
                        panic_guard.base_ptr = ptr_i;
                    }
                    ptr_i.write(value_i);
                    if $D > 0 {
                        ptr_i = ptr_i.add(1);
                    }
                }
                // From now on, the code can no longer `panic!`, let's take the
                // symbolic ownership back
                core::mem::forget(panic_guard);

                positive_variant!(array.assume_init(), $residue)
            } // end unsafe
        } // end if/else !core::mem::needs_drop::<$T>()
    } // end macro arm
} // end base_array_init_impl

// Right now we just export all macros so higher up, all the caller has to do is `use base::*` then
// they can use `base_array_init_impl!` directly. There are definitly downsides to this approach,
// namely that it pollutes the namespace internally (still not visible outside of the crate), but
// it allows us to avoid implementing a TT-muncher which would be a lot more complicated.
pub(crate) use prop_negative;
pub(crate) use should_await;
pub(crate) use positive_variant;
pub(crate) use base_array_init_impl;

