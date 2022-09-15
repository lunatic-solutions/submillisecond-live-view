//! `Cd`: A "smart pointer" that tracks changes to the data it owns.
//!
//! ## Usage
//! ```
//! use changed::Cd;
//!
//! // Create the change tracker with an i32
//! let mut test: Cd<i32> = Cd::new(20);
//!
//! // Mutate it (calling deref_mut through the *)
//! *test += 5;
//!
//! // changed() reports whether or not it was changed
//! assert!(test.changed());
//!
//! // Reset the tracker back to false
//! test.reset();
//!
//! // Read the data
//! assert_eq!(*test, 25);
//!
//! // That didn't trip the change detection!
//! assert!(!test.changed());
//! ```
//!
//! ## How it works
//! Technically, it doesn't track changes. It tracks calls to `deref_mut()`
//! so it is entirely possible to call `deref_mut()` and not change it, giving a false positive.
//!
//! Along with that, there is a function to mutate a `Cd` without tripping change detection.

use std::ops::{Deref, DerefMut};

/// Cd: Change Detection
///
/// Start by creating one with [`new()`](Cd::new()).
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cd<T> {
    data: T,
    changed: bool,
}

impl<T> Cd<T> {
    /// Create a new Cd with data.
    /// It is initialized to false for change detection.
    ///
    /// ```
    /// use changed::Cd;
    /// let cd = Cd::new(5);
    /// ```
    pub fn new(data: T) -> Cd<T> {
        Cd {
            data,
            changed: false,
        }
    }

    /// Create a new Cd with data.
    /// It is initialized to true for change detection.
    /// ```
    /// use changed::Cd;
    /// let cd = Cd::new_true(5);
    /// assert!(cd.changed());
    /// ```
    pub fn new_true(data: T) -> Cd<T> {
        Cd {
            data,
            changed: true,
        }
    }

    /// Reset the change tracking to false.
    /// ```
    /// use changed::Cd;
    /// let mut cd = Cd::new_true(5);
    /// cd.reset();
    /// assert!(!cd.changed());
    /// ```
    pub fn reset(&mut self) {
        self.changed = false;
    }

    /// Take the data out of the Cd.
    /// Consumes self and returns data.
    /// ```
    /// use changed::Cd;
    /// let cd = Cd::new(5);
    /// let data = cd.take();
    /// // Error: cd has been moved.
    /// // cd.changed();
    /// ```
    pub fn take(self) -> T {
        self.data
    }

    /// Check if the Cd has been changed since the last call to reset (or created.)
    /// ```
    /// use changed::Cd;
    /// let mut cd = Cd::new(5);
    /// assert!(!cd.changed());
    /// *cd += 5;
    /// assert!(cd.changed());
    /// ```
    pub fn changed(&self) -> bool {
        self.changed
    }

    /// Mutate the Cd without tripping change detection.
    ///
    /// ```
    /// use changed::Cd;
    /// let mut cd = Cd::new(5);
    /// *cd.mutate_silently() += 5;
    /// assert!(!cd.changed());
    /// ```
    pub fn mutate_silently(&mut self) -> &mut T {
        &mut self.data
    }
}

/// deref does not trip change detection.
/// ```
/// use changed::Cd;
/// let cd = Cd::new(5);
/// assert_eq!(*cd, 5); // deref for == 5
/// assert!(!cd.changed()); // .changed() is false
/// ```
impl<T> Deref for Cd<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

/// deref_mut trips change detection.
/// ```
/// use changed::Cd;
/// let mut cd = Cd::new(5);
/// *cd += 5; // deref_mut for add assign
/// assert_eq!(*cd, 10);
/// assert!(cd.changed()); // .changed() is true
/// ```
impl<T> DerefMut for Cd<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.changed = true;
        &mut self.data
    }
}

/// Impl default where the data impls default. Change detection is initialized to false.
/// ```
/// use changed::Cd;
/// // 0 is default for i32.
/// let zero: Cd<i32> = Cd::default();
/// assert!(!zero.changed());
/// ```
impl<T: Default> Default for Cd<T> {
    fn default() -> Self {
        Cd::new(T::default())
    }
}

#[cfg(test)]
mod tests {
    use super::Cd;

    #[test]
    fn it_works() {
        let mut changed = Cd::new(15);
        *changed += 5;
        assert!(changed.changed());
        changed.reset();
        assert_eq!(*changed, 20);
        assert!(!changed.changed());
    }
}
