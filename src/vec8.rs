//! A vector that is indexed by `u8` instead of `usize`.

// Copyright 2017 Matt Brubeck.  Copyright 2014 The Rust Project Developers. See the COPYRIGHT file
// at the top-level directory of this distribution and at http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <http://www.apache.org/licenses/LICENSE-2.0> or
// the MIT license <http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.


use std::marker::PhantomData;
use std::{cmp, fmt, iter, mem, ops, ptr, slice, u8, vec};
use std::ptr::NonNull;

/// A vector that is indexed by `u8` instead of `usize`.
///
/// On 32-bit platforms, `Vec8<T>` is mostly identical to the standard library `Vec<T>`.
///
/// On 64-bit platforms, the `Vec8<T>` struct takes up less space than the standard `Vec<T>`
/// struct (16 bytes instead of 24 bytes), but its maximum capacity is `u8::MAX` instead of
/// `usize::MAX`.
///
/// ## Warning
///
/// This type does not yet support 16-bit or 8-bit platforms. It may cause undefined behavior
/// if used on any architecture with addresses smaller than 32 bits.
///
/// ## Examples
///
/// ```
/// use mediumvec::Vec8;
///
/// let mut vec = Vec8::new();
/// vec.push(1);
/// vec.push(2);
///
/// assert_eq!(vec.len(), 2);
/// assert_eq!(vec[0], 1);
///
/// assert_eq!(vec.pop(), Some(2));
/// assert_eq!(vec.len(), 1);
///
/// vec[0] = 7;
/// assert_eq!(vec[0], 7);
///
/// vec.extend([1, 2, 3].iter().cloned());
///
/// assert_eq!(vec, [7, 1, 2, 3]);
/// ```
///
/// The `vec32!` macro provides convenient initialization:
///
/// ```
/// #[macro_use] extern crate mediumvec;
///
/// fn main() {
///     let mut vec = vec32![1, 2, 3];
///     assert_eq!(vec, [1, 2, 3]);
///
///     let vec = vec32![0; 5];
///     assert_eq!(vec, vec32![0, 0, 0, 0, 0]);
/// }
/// ```
#[repr(C, packed)]
pub struct Vec8<T> {
    ptr: ptr::NonNull<T>,
    cap: u8,
    len: u8,
}

use serde::ser::{Serialize, Serializer, SerializeSeq};
use serde::de::{Deserialize, Deserializer, SeqAccess, Visitor};

impl<T> Serialize for Vec8<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for element in self {
            seq.serialize_element(element)?;
        }
        seq.end()
    }
}

struct MyVec8Visitor<K> {
    marker: PhantomData<fn() -> Vec8<K>>
}

impl<K> MyVec8Visitor<K> {
    fn new() -> Self {
        MyVec8Visitor {
            marker: PhantomData
        }
    }
}

impl<'de, K> Visitor<'de> for MyVec8Visitor<K>
where
    K: Deserialize<'de>
{
    type Value = Vec8<K>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a Vec8")
    }

    fn visit_seq<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where M: SeqAccess<'de>,
    {
        let mut vec = Vec::with_capacity(access.size_hint().unwrap_or(0));

        while let Some(value) = access.next_element()? {
            vec.push(value);
        }

        let mut vec8 = Vec8::with_capacity(vec.len() as u8);
        for v in vec {
            vec8.push(v);
        }

        Ok(vec8)
    }
}

impl<'de, T> Deserialize<'de> for Vec8<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        deserializer.deserialize_seq(MyVec8Visitor::new())
    }
}

unsafe impl<T> Send for Vec8<T> where T: Send + Sized {}
unsafe impl<T> Sync for Vec8<T> where T: Sync + Sized {}

impl<T> Vec8<T> {
    /// Constructs a new, empty vector.
    ///
    /// The vector will not allocate until elements are pushed onto it.
    pub fn new() -> Vec8<T> {
        Vec8 {
            ptr: NonNull::dangling(),
            cap: if mem::size_of::<T>() == 0 { u8::MAX } else { 0 },
            len: 0,
        }
    }

    /// Constructs a new, empty (length 0) vector with the specified capacity.
    pub fn with_capacity(cap: u8) -> Vec8<T> {
        let mut v = Vec::with_capacity(cap as usize);
        let ptr = NonNull::new(v.as_mut_ptr()).unwrap();
        mem::forget(v);

        Vec8 { ptr, cap, len: 0 }
    }

    /// Append an element to the vector.
    ///
    /// Panics if the number of elements in the vector overflows `u8`.
    pub fn push(&mut self, value: T) {
        if self.len == self.cap {
            self.reserve(1);
        }
        unsafe {
            let end = self.as_mut_ptr().offset(self.len as isize);
            ptr::write(end, value);
            self.len += 1;
        }
    }

    /// Remove the last element from a vector and return it, or `None` if it is empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            unsafe {
                self.len -= 1;
                Some(ptr::read(self.get_unchecked(self.len as usize)))
            }
        }
    }

    /// Remove and return the element at position `index`, shifting elements after it to the left.
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// ## Examples
    ///
    /// ```
    /// # #[macro_use] extern crate mediumvec;
    /// # fn main() {
    /// let mut v = vec32![1, 2, 3];
    /// assert_eq!(v.remove(1), 2);
    /// assert_eq!(v, [1, 3]);
    /// # }
    /// ```
    pub fn remove(&mut self, index: u8) -> T {
        let len = self.len;
        assert!(index < len);
        unsafe {
            let ptr = self.as_mut_ptr().offset(index as isize);
            let ret = ptr::read(ptr);
            ptr::copy(ptr.offset(1), ptr, (len - index - 1) as usize);
            self.len -= 1;
            ret
        }
    }

    /// Insert an element at position `index`, shifting elements after it to the right.
    ///
    /// Panics if `index` is out of bounds or the length of the vector overflows `u8`.
    ///
    /// ## Examples
    ///
    /// ```
    /// # #[macro_use] extern crate mediumvec;
    /// # fn main() {
    /// let mut vec = vec![1, 2, 3];
    /// vec.insert(1, 4);
    /// assert_eq!(vec, [1, 4, 2, 3]);
    /// vec.insert(4, 5);
    /// assert_eq!(vec, [1, 4, 2, 3, 5]);
    /// # }
    /// ```
    pub fn insert(&mut self, index: u8, element: T) {
        let len = self.len;
        assert!(index <= len);
        if len == self.cap {
            self.reserve(1);
        }

        unsafe {
            let p = self.as_mut_ptr().offset(index as isize);
            ptr::copy(p, p.offset(1), (len - index) as usize);
            ptr::write(p, element);
            self.len += 1;
        }
    }

    /// Reserve capacity for at least `additional` more elements to be inserted.
    ///
    /// May reserve more space than requested, to avoid frequent reallocations.
    ///
    /// Panics if the new capacity overflows `u8`.
    ///
    /// Re-allocates only if `self.capacity() < self.len() + additional`.
    pub fn reserve(&mut self, additional: u8) {
        let min_cap = self.len.checked_add(additional).expect("capacity overflow");
        if min_cap <= self.cap {
            return
        }
        let double_cap = self.cap.saturating_mul(2);
        let new_cap = cmp::max(min_cap, double_cap);
        let additional = new_cap - self.len;
        self.reserve_exact(additional);
    }

    /// Reserves the minimum capacity for `additional` more elements to be inserted.
    ///
    /// Panics if the new capacity overflows `u8`.
    ///
    /// Re-allocates only if `self.capacity() < self.len() + additional`.
    pub fn reserve_exact(&mut self, additional: u8) {
        self.as_vec(|v| v.reserve_exact(additional as usize));
    }

    /// Converts a `Vec<T>` to a `Vec8<T>`.
    ///
    /// Panics if the vector's length is greater than `u8::MAX`.
    ///
    /// Re-allocates only if the vector's capacity is greater than `u8::MAX`.
    pub fn from_vec(mut vec: Vec<T>) -> Vec8<T> {
        let len = vec.len();
        assert!(len <= u8::MAX as usize);

        if vec.capacity() > u8::MAX as usize {
            vec.shrink_to_fit();
        }

        let cap = if mem::size_of::<T>() == 0 {
            u8::MAX
        } else {
            vec.capacity() as u8
        };

        let ptr = NonNull::new(vec.as_mut_ptr()).unwrap();
        mem::forget(vec);

        Vec8 { ptr, cap, len: len as u8 }
    }

    /// Convert a `Vec8<T>` into a `Vec<T>` without re-allocating.
    pub fn into_vec(self) -> Vec<T> {
        unsafe {
            Vec::from_raw_parts(self.ptr.as_ptr(), self.len as usize, self.cap as usize)
        }
    }

    /// Convert a `Vec8<T>` into a `Vec<T>`, mutate it, then convert it back.
    ///
    /// This is a convenient way to call `Vec` methods that don't have `Vec8` equivalents.
    ///
    /// Panics if the vector's length increases to greater than `u8::MAX`.
    ///
    /// ```
    /// # #[macro_use] extern crate mediumvec;
    /// # fn main() {
    /// let mut v = vec32![0, 0, 0, 1, 1, 2, 3, 3, 3];
    /// v.as_vec(|vec| vec.dedup());
    /// assert_eq!(v, [0, 1, 2, 3]);
    /// # }
    /// ```
    pub fn as_vec<F>(&mut self, f: F) where F: FnOnce(&mut Vec<T>) {
        let mut vec = mem::replace(self, Vec8::new()).into_vec();
        f(&mut vec);
        *self = Vec8::from_vec(vec);
    }

    /// Returns the maximum number of elements the vector can hold without reallocating.
    pub fn capacity(&self) -> u8 {
        self.cap
    }

    /// Clears the vector, removing all values.
    ///
    /// Note that this method has no effect on the allocated capacity
    /// of the vector.
    pub fn clear(&mut self) {
        self.truncate(0)
    }

    /// Shorten the vector, keeping the first `len` elements and dropping the rest.
    ///
    /// If `len` is greater than the vector's current length, this has no effect.
    pub fn truncate(&mut self, len: u8) {
        unsafe {
            // drop any extra elements
            while len < self.len {
                // decrement len before the drop_in_place(), so a panic on Drop
                // doesn't re-drop the just-failed value.
                self.len -= 1;
                let len = self.len as usize;
                ptr::drop_in_place(self.get_unchecked_mut(len));
            }
        }
    }
}

/// Initialize a `Vec8`.
///
/// ## Examples
///
/// ```
/// #[macro_use] extern crate mediumvec;
///
/// # fn main() {
/// let mut vec = vec32![1, 2, 3];
/// vec.push(4);
/// assert_eq!(vec, [1, 2, 3, 4]);
///
/// let vec = vec32![0; 5];
/// assert_eq!(vec, [0, 0, 0, 0, 0]);
/// # }
/// ```
// #[macro_export]
// macro_rules! vec32 {
//     ($elem:expr; $n:expr) => (
//         $crate::Vec8::from_vec(vec![$elem; $n])
//     );
//     ($($x:expr),*) => (
//         $crate::Vec8::from_vec(vec![$($x),*])
//     );
//     ($($x:expr,)*) => (vec32![$($x),*])
// }

// Trait implementations:

impl<T> Drop for Vec8<T> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(&mut self[..]);
            Vec::from_raw_parts(self.ptr.as_ptr(), 0, self.cap as usize);
        }
    }
}

impl<T: Clone> Clone for Vec8<T> {
    fn clone(&self) -> Self {
        Vec8::from_vec(self[..].to_vec())
    }
}

impl<T> ops::Deref for Vec8<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe {
            slice::from_raw_parts(self.ptr.as_ptr(), self.len as usize)
        }
    }
}

impl<T> ops::DerefMut for Vec8<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe {
            slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len as usize)
        }
    }
}

impl<T> IntoIterator for Vec8<T> {
    type Item = T;
    type IntoIter = vec::IntoIter<T>;

    fn into_iter(self) -> vec::IntoIter<T> {
        self.into_vec().into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Vec8<T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> slice::Iter<'a, T> {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut Vec8<T> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    fn into_iter(self) -> slice::IterMut<'a, T> {
        self.iter_mut()
    }
}

impl<T> Extend<T> for Vec8<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let iterator = iter.into_iter();
        let (lower, _) = iterator.size_hint();
        assert!(lower < u8::MAX as usize);
        self.reserve(lower as u8);

        for i in iterator {
            self.push(i);
        }
    }
}

impl<T> iter::FromIterator<T> for Vec8<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Vec8<T> {
        let iterator = iter.into_iter();
        let (lower, _) = iterator.size_hint();
        assert!(lower < u8::MAX as usize);

        let mut v = Vec8::with_capacity(lower as u8);
        for i in iterator {
            v.push(i);
        }
        v
    }
}

impl<T: fmt::Debug> fmt::Debug for Vec8<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self[..], f)
    }
}

impl<T: PartialOrd> PartialOrd for Vec8<T> {
    fn partial_cmp(&self, other: &Vec8<T>) -> Option<cmp::Ordering> {
        PartialOrd::partial_cmp(&**self, &**other)
    }
}

impl<T: Eq> Eq for Vec8<T> {}

impl<T, U> PartialEq<U> for Vec8<T> where U: for<'a> PartialEq<&'a [T]> {
    fn eq(&self, other: &U) -> bool { *other == &self[..] }
    fn ne(&self, other: &U) -> bool { *other != &self[..] }
}

#[cfg(test)]
mod tests {
    use super::Vec8;

    #[test]
    fn it_works() {
        let mut v = vec32![1, 2, 3];
        assert_eq!(v.pop(), Some(3));
        v.push(4);
        assert_eq!(v, vec![1, 2, 4]);
    }

    #[test]
    fn test_size() {
        use std::mem::size_of;
        #[cfg(target_pointer_width = "64")]
        assert_eq!(size_of::<Vec8<()>>(), 16);
        #[cfg(target_pointer_width = "32")]
        assert_eq!(size_of::<Vec8<()>>(), 12);
    }
}
