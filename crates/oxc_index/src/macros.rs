/// A macro equivalent to the stdlib's `vec![]`, but producing an `IndexVec`.
#[macro_export]
macro_rules! index_vec {
    ($($tokens:tt)*) => {
        $crate::IndexVec::from_vec(vec![$($tokens)*])
    }
}

/// A macro similar to the stdlib's `vec![]`, but producing an
/// `Box<IndexSlice<I, [T]>>` (That is, an `IndexBox<I, [T]>`).
#[macro_export]
macro_rules! index_box {
    ($($tokens:tt)*) => {
        $crate::IndexVec::from_vec(vec![$($tokens)*]).into_boxed_slice()
    }
}

/// A macro equivalent to the stdlib's `vec![]`, but producing an `IndexVec`.
#[macro_export]
macro_rules! non_zero_index_vec {
    ($($tokens:tt)*) => {
        $crate::NonZeroIndexVec::from_vec(vec![$($tokens)*])
    }
}

/// A macro similar to the stdlib's `vec![]`, but producing an
/// `Box<IndexSlice<I, [T]>>` (That is, an `IndexBox<I, [T]>`).
#[macro_export]
macro_rules! non_zero_index_box {
    ($($tokens:tt)*) => {
        $crate::NonZeroIndexVec::from_vec(vec![$($tokens)*]).into_boxed_slice()
    }
}

/// Generate the boilerplate for a newtyped index struct, for use with
/// `IndexVec`.
///
/// In the future, if the compile-time overhead of doing so is reduced, this may
/// be replaced with a proc macro.
///
/// ## Usage
///
/// ### Standard
///
/// The rough usage pattern of this macro is:
///
/// ```rust,no_run
/// index_vec::define_index_type! {
///     // Note that isn't actually a type alias, `MyIndex` is
///     // actually defined as a struct. XXX is this too confusing?
///     pub struct MyIndex = u32;
///     // optional extra configuration here of the form:
///     // `OPTION_NAME = stuff;`
///     // See below for details.
/// }
/// ```
///
/// Note that you can use other index types than `u32`, and you can set it to be
/// `MyIndex(pub u32)` as well. Currently, the wrapped item be a tuple struct,
/// however (patches welcome).
///
/// ### Customization
///
/// After the struct declaration, there are a number of configuration options
/// the macro uses to customize how the type it generates behaves. For example:
///
/// ```rust,no_run
/// index_vec::define_index_type! {
///     pub struct Span = u32;
///
///     // Don't allow any spans with values higher this.
///     MAX_INDEX = 0x7fff_ff00;
///
///     // But I also am not too worried about it, so only
///     // perform the asserts in debug builds.
///     DISABLE_MAX_INDEX_CHECK = cfg!(not(debug_assertions));
/// }
/// ```
///
/// ## Configuration options
///
/// This macro has a few ways you can customize it's output behavior. There's
/// not really any great syntax I can think of for them, but, well.
///
/// #### `MAX_INDEX = <expr producing usize>`
///
/// Assert if anything tries to construct an index above that value.
///
/// By default, this is `$raw_type::max_value() as usize`, e.g. we check that
/// our cast from `usize` to our wrapper is lossless, but we assume any all
/// instance of `$raw_type` is valid in this index domain.
///
/// Note that these tests can be disabled entirely, or conditionally, with
/// `DISABLE_MAX_INDEX_CHECK`. Additionally, the generated type has
/// `from_usize_unchecked` and `from_raw_unchecked` functions which can be used
/// to ignore these checks.
///
/// #### `DISABLE_MAX_INDEX_CHECK = <expr>;`
///
/// Set to true to disable the assertions mentioned above. False by default.
///
/// To be clear, if this is set to false, we blindly assume all casts between
/// `usize` and `$raw_type` succeed.
///
/// A common use is setting `DISABLE_MAX_INDEX_CHECK = !cfg!(debug_assertions)`
/// to avoid the tests at compile time
///
/// For the sake of clarity, disabling this cannot lead to memory unsafety -- we
/// still go through bounds checks when accessing slices, and no unsafe code
/// should rely on on these checks (unless you write some, and don't! only use
/// this for correctness!).
///
/// #### `DEFAULT = <expr>;`
/// If provided, we'll implement `Default` for the index type using this
/// expresson.
///
/// Example:
///
/// ```rust,no_run
/// index_vec::define_index_type! {
///     pub struct MyIdx = u16;
///     MAX_INDEX = (u16::max_value() - 1) as usize;
///     // Set the default index to be an invalid index, as
///     // a hacky way of having this type behave somewhat
///     // like it were an Option<MyIdx> without consuming
///     // extra space.
///     DEFAULT = (MyIdx::from_raw_unchecked(u16::max_value()));
/// }
/// ```
///
/// #### `DEBUG_FORMAT = <expr>;`
///
/// By default we write the underlying integer out in a Debug implementation
/// with `{:?}`. Sometimes you'd like more info though. For example, the type of
/// the index. This can be done via `DEBUG_FORMAT`:
///
/// ```rust
/// index_vec::define_index_type! {
///     struct FooIdx = usize;
///     DEBUG_FORMAT = "Foo({})";
/// }
/// // Then ...
/// # fn main() {
/// let v = FooIdx::new(10);
/// assert_eq!("Foo(10)", format!("{:?}", v));
/// # }
/// ```
///
/// #### `DISPLAY_FORMAT = <expr>;`
///
/// Similarly to `DEBUG_FORMAT`, we can implement Display for you. Unlike
/// `DEBUG_FORMAT`, if you do not set this, we will not implement `Display` for
/// the index type.
///
/// ```rust
/// index_vec::define_index_type! {
///     struct FooIdx = usize;
///     DISPLAY_FORMAT = "{}";
///     // Note that you can use both DEBUG_FORMAT and DISPLAY_FORMAT.
///     DEBUG_FORMAT = "#<foo {}>";
/// }
/// // Then ...
/// # fn main() {
/// let v = FooIdx::new(10);
/// assert_eq!("10", format!("{}", v));
/// assert_eq!("#<foo 10>", format!("{:?}", v));
/// # }
/// ```
///
/// #### `IMPL_RAW_CONVERSIONS = true;`
///
/// We always automatically implement `From<usize> for YourIndex` and
/// `From<YourIndex> for usize`. We don't do this for the "raw" type (e.g. `u32`
/// if your type is declared as `struct FooIdx = u32;`), unless you request it
/// via this option. It's an error to use this if your raw type is usize.
///
/// ```rust
/// index_vec::define_index_type! {
///     struct FooIdx = u32;
///     IMPL_RAW_CONVERSIONS = true;
/// }
///
/// # fn main() {
/// let as_index = FooIdx::from(5u32);
/// let as_u32 = u32::from(as_index);
/// assert_eq!(as_u32, 5);
/// # }
/// ```
#[macro_export]
macro_rules! define_index_type {
    // public api for creating new NonZeroIdx types.
    // keep in mind the `#[non_zero]` attribute should come before others.
    (
        #[non_zero]
        $(#[$attrs:meta])*
        $v:vis struct $type:ident = $primitive:tt;
        $($CONFIG_NAME:ident = $value:expr;)* $(;)?
    ) => {
        $crate::__define_index_type_inner!{
            @configs [$(($CONFIG_NAME; $value))*]
            @attrs [$(#[$attrs])*]
            @derives [#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]]
            @decl [$v struct $type ($crate::non_zero!($primitive))]
            @debug_fmt ["{}"]
            @max [(<$crate::non_zero!($primitive)>::MAX.get() as usize)]
            @no_check_max [false]
            @non_zero [$primitive]
        }
    };
    // public api for creating new Idx types.
    (
        $(#[$attrs:meta])*
        $v:vis struct $type:ident = $raw:ty;
        $($CONFIG_NAME:ident = $value:expr;)* $(;)?
    ) => {
        $crate::__define_index_type_inner!{
            @configs [$(($CONFIG_NAME; $value))*]
            @attrs [$(#[$attrs])*]
            @derives [#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]]
            @decl [$v struct $type ($raw)]
            @debug_fmt ["{}"]
            @max [(<$raw>::max_value() as usize)]
            @no_check_max [false]
            @non_zero [false]
        }
    };
}

/// Wraps the given type in a `NonZero*` type.
///
/// TODO: replace me with `NonZero<T>` when it stabilized.
#[macro_export]
#[doc(hidden)]
macro_rules! non_zero {
    (u8) => {
        std::num::NonZeroU8
    };
    (u16) => {
        std::num::NonZeroU16
    };
    (u32) => {
        std::num::NonZeroU32
    };
    (u64) => {
        std::num::NonZeroU64
    };
    (u128) => {
        std::num::NonZeroU128
    };
    (usize) => {
        std::num::NonZeroUsize
    };
    (i8) => {
        std::num::NonZeroI8
    };
    (i16) => {
        std::num::NonZeroI16
    };
    (i32) => {
        std::num::NonZeroI32
    };
    (i64) => {
        std::num::NonZeroI64
    };
    (i128) => {
        std::num::NonZeroI128
    };
    (isize) => {
        std::num::NonZeroIsize
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! unknown_define_index_type_option {
    () => {};
}

#[cfg(feature = "serialize")]
#[macro_export]
#[doc(hidden)]
macro_rules! __internal_maybe_index_impl_serde {
    ($type:ident) => {
        impl serde::ser::Serialize for $type {
            fn serialize<S: serde::ser::Serializer>(
                &self,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                self.index().serialize(serializer)
            }
        }

        impl<'de> serde::de::Deserialize<'de> for $type {
            fn deserialize<D: serde::de::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                usize::deserialize(deserializer).map(Self::from_usize)
            }
        }
    };
}

#[cfg(not(feature = "serialize"))]
#[macro_export]
#[doc(hidden)]
macro_rules! __internal_maybe_index_impl_serde {
    ($type:ident) => {};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __internal_define_index_type_inner_impl_generic {
    (@type[$primitive:ty] $v:vis $type:ident($raw:ty)) => {
        $crate::__internal_define_non_zero_index_type_inner_impl!(@type($type, $v, $raw, $primitive));
    };
    (@type[false] $v:vis $type:ident($raw:ty)) => {
        $crate::__internal_define_index_type_inner_impl!(@type($type, $v, $raw));
    };

    (@trait[$primitive:ty] $v:vis $type:ident($raw:ty)) => {
        $crate::__internal_define_non_zero_index_type_inner_impl!(@trait($type, $v, $raw, $primitive));
    };
    (@trait[false] $v:vis $type:ident($raw:ty)) => {
        $crate::__internal_define_index_type_inner_impl!(@trait($type, $v, $raw));
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __internal_define_index_type_inner_impl {
    (@type($type:ident, $v:vis, $raw:ty)) => {
            const _ENSURE_RAW_IS_UNSIGNED: [(); 0] = [(); <$raw>::min_value() as usize];

            /// Construct this index type from the wrapped integer type.
            #[inline(always)]
            $v fn from_raw(value: $raw) -> Self {
                Self::from_usize(value as usize)
            }

            /// Construct from a usize without any checks.
            #[inline(always)]
            $v const fn from_usize_unchecked(value: usize) -> Self {
                Self { _raw: value as $raw }
            }

            /// Construct this index type from a usize.
            #[inline]
            $v fn from_usize(value: usize) -> Self {
                Self::check_index(value as usize);
                Self { _raw: value as $raw }
            }

            /// Get the wrapped index as a usize.
            #[inline(always)]
            $v const fn index(self) -> usize {
                self._raw as usize
            }
    };

    (@trait($type:ident, $v:vis, $raw:ty)) => {
        impl $crate::Idx for $type {
            #[inline]
            fn from_usize(value: usize) -> Self {
                Self::from(value)
            }

            #[inline]
            fn index(self) -> usize {
                usize::from(self)
            }
        }

    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __internal_define_non_zero_index_type_inner_impl {
    (@type($type:ident, $v:vis, $raw:ty, $primitive:ty)) => {
            const _ENSURE_RAW_IS_UNSIGNED: [(); 1] = [(); <$raw>::MIN.get() as usize];

            /// Construct this index type from the wrapped integer type.
            #[inline(always)]
            $v fn from_raw(value: $raw) -> Self {
                Self::from_usize(value.get() as usize)
            }

            /// Construct from a usize without any checks.
            #[inline(always)]
            $v const unsafe fn from_usize_unchecked(value: usize) -> Self {
                Self { _raw: <$raw>::new_unchecked(value as $primitive) }
            }

            /// Construct this index type from a usize.
            #[inline]
            $v fn from_usize(value: usize) -> Self {
                Self::check_index(value as usize);
                Self {
                    _raw:
                        <$raw>::new(value as $primitive)
                        .expect("Attempt to initialize a non-zero index with a value of `zero`.")
                }
            }

            /// Get the wrapped index as a usize.
            #[inline(always)]
            $v const fn index(self) -> usize {
                self._raw.get() as usize
            }
    };

    (@trait($type:ident, $v:vis, $raw:ty, $primitive:ty)) => {
        impl $crate::NonZeroIdx for $type {
            #[inline]
            fn from_usize(value: usize) -> Self {
                Self::from(value)
            }

            #[inline]
            fn index(self) -> usize {
                usize::from(self)
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __define_index_type_inner {
    // DISABLE_MAX_INDEX_CHECK
    (
        @configs [(DISABLE_MAX_INDEX_CHECK; $no_check_max:expr) $(($CONFIG_NAME:ident; $value:expr))*]
        @attrs [$(#[$attrs:meta])*]
        @derives [$(#[$derive:meta])*]
        @decl [$v:vis struct $type:ident ($raw:ty)]
        @debug_fmt [$dbg:expr]
        @max [$max:expr]
        @no_check_max [$_old_no_check_max:expr]
        @non_zero [$non_zero:tt]
    ) => {
        $crate::__define_index_type_inner!{
            @configs [$(($CONFIG_NAME; $value))*]
            @attrs [$(#[$attrs])*]
            @derives [$(#[$derive])*]
            @decl [$v struct $type ($raw)]
            @debug_fmt [$dbg]
            @max [$max]
            @no_check_max [$no_check_max]
            @non_zero [$non_zero]
        }
    };

    // MAX_INDEX
    (
        @configs [(MAX_INDEX; $new_max:expr) $(($CONFIG_NAME:ident; $value:expr))*]
        @attrs [$(#[$attrs:meta])*]
        @derives [$(#[$derive:meta])*]
        @decl [$v:vis struct $type:ident ($raw:ty)]
        @debug_fmt [$dbg:expr]
        @max [$max:expr]
        @no_check_max [$cm:expr]
        @non_zero [$non_zero:tt]
    ) => {
        $crate::__define_index_type_inner!{
            @configs [$(($CONFIG_NAME; $value))*]
            @attrs [$(#[$attrs])*]
            @derives [$(#[$derive])*]
            @decl [$v struct $type ($raw)]
            @debug_fmt [$dbg]
            @max [$new_max]
            @no_check_max [$cm]
            @non_zero [$non_zero]
        }
    };

    // DEFAULT
    (
        @configs [(DEFAULT; $default_expr:expr) $(($CONFIG_NAME:ident; $value:expr))*]
        @attrs [$(#[$attrs:meta])*]
        @derives [$(#[$derive:meta])*]
        @decl [$v:vis struct $type:ident ($raw:ty)]
        @debug_fmt [$dbg:expr]
        @max [$max:expr]
        @no_check_max [$no_check_max:expr]
        @non_zero [$non_zero:tt]
    ) => {
        $crate::__define_index_type_inner!{
            @configs [$(($CONFIG_NAME; $value))*]
            @attrs [$(#[$attrs])*]
            @derives [$(#[$derive])*]
            @decl [$v struct $type ($raw)]
            @debug_fmt [$dbg]
            @max [$max]
            @no_check_max [$no_check_max]
            @non_zero [$non_zero]
        }
        impl Default for $type {
            #[inline]
            fn default() -> Self {
                $default_expr
            }
        }
    };

    // DEBUG_FORMAT
    (
        @configs [(DEBUG_FORMAT; $dbg:expr) $(($CONFIG_NAME:ident; $value:expr))*]
        @attrs [$(#[$attrs:meta])*]
        @derives [$(#[$derive:meta])*]
        @decl [$v:vis struct $type:ident ($raw:ty)]
        @debug_fmt [$old_dbg:expr]
        @max [$max:expr]
        @no_check_max [$no_check_max:expr]
        @non_zero [$non_zero:tt]
    ) => {
        $crate::__define_index_type_inner!{
            @configs [$(($CONFIG_NAME; $value))*]
            @attrs [$(#[$attrs])*]
            @derives [$(#[$derive])*]
            @decl [$v struct $type ($raw)]
            @debug_fmt [$dbg]
            @max [$max]
            @no_check_max [$no_check_max]
            @non_zero [$non_zero]
        }
    };

    // DISPLAY_FORMAT
    (
        @configs [(DISPLAY_FORMAT; $format:expr) $(($CONFIG_NAME:ident; $value:expr))*]
        @attrs [$(#[$attrs:meta])*]
        @derives [$(#[$derive:meta])*]
        @decl [$v:vis struct $type:ident ($raw:ty)]
        @debug_fmt [$dbg:expr]
        @max [$max:expr]
        @no_check_max [$no_check_max:expr]
        @non_zero [$non_zero:tt]
    ) => {
        $crate::__define_index_type_inner!{
            @configs [$(($CONFIG_NAME; $value))*]
            @attrs [$(#[$attrs])*]
            @derives [$(#[$derive])*]
            @decl [$v struct $type ($raw)]
            @debug_fmt [$dbg]
            @max [$max]
            @no_check_max [$no_check_max]
            @non_zero [$non_zero]
        }

        impl core::fmt::Display for $type {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, $format, self.index())
            }
        }
    };

    // IMPL_RAW_CONVERSIONS
    (
        @configs [(IMPL_RAW_CONVERSIONS; $val:expr) $(($CONFIG_NAME:ident; $value:expr))*]
        @attrs [$(#[$attrs:meta])*]
        @derives [$(#[$derive:meta])*]
        @decl [$v:vis struct $type:ident ($raw:ty)]
        @debug_fmt [$dbg:expr]
        @max [$max:expr]
        @no_check_max [$no_check_max:expr]
        @non_zero [$non_zero:tt]
    ) => {
        $crate::__define_index_type_inner!{
            @configs [$(($CONFIG_NAME; $value))*]
            @attrs [$(#[$attrs])*]
            @derives [$(#[$derive])*]
            @decl [$v struct $type ($raw)]
            @debug_fmt [$dbg]
            @max [$max]
            @no_check_max [$no_check_max]
            @non_zero [$non_zero]
        }
        // Ensure they passed in true. This is... cludgey.
        const _: [(); 1] = [(); $val as usize];

        impl From<$type> for $raw {
            #[inline]
            fn from(v: $type) -> $raw {
                v.raw()
            }
        }

        impl From<$raw> for $type {
            #[inline]
            fn from(value: $raw) -> Self {
                Self::from_raw(value)
            }
        }
    };
    // Try to make rust emit a decent error message...
    (
        @configs [($other:ident; $format:expr) $(($CONFIG_NAME:ident; $value:expr))*]
        @attrs [$(#[$attrs:meta])*]
        @derives [$(#[$derive:meta])*]
        @decl [$v:vis struct $type:ident ($raw:ty)]
        @debug_fmt [$dbg:expr]
        @max [$max:expr]
        @no_check_max [$no_check_max:expr]
        @non_zero [$non_zero:tt]
    ) => {
        $crate::unknown_define_index_type_option!($other);
    };
    // finish
    (
        @configs []
        @attrs [$(#[$attrs:meta])*]
        @derives [$(#[$derive:meta])*]
        @decl [$v:vis struct $type:ident ($raw:ty)]
        @debug_fmt [$dbg:expr]
        @max [$max:expr]
        @no_check_max [$no_check_max:expr]
        @non_zero [$non_zero:tt]
    ) => {

        $(#[$derive])*
        $(#[$attrs])*
        #[repr(transparent)]
        $v struct $type { _raw: $raw }

        // mixin the type specific trait implementations
        $crate::__internal_define_index_type_inner_impl_generic!(@trait[$non_zero] $v $type($raw));

        impl $type {
            // mixin the type specific implementation
            $crate::__internal_define_index_type_inner_impl_generic!(@type[$non_zero] $v $type($raw));

            /// If `Self::CHECKS_MAX_INDEX` is true, we'll assert if trying to
            /// produce a value larger than this in any of the ctors that don't
            /// have `unchecked` in their name.
            $v const MAX_INDEX: usize = $max;

            /// Does this index type assert if asked to construct an index
            /// larger than MAX_INDEX?
            $v const CHECKS_MAX_INDEX: bool = !$no_check_max;

            /// Construct this index type from a usize. Alias for `from_usize`.
            #[inline(always)]
            $v fn new(value: usize) -> Self {
                Self::from_usize(value)
            }

            /// Construct this index type from one in a different domain
            #[inline(always)]
            $v fn from_foreign<F: $crate::Idx>(value: F) -> Self {
                Self::from_usize(value.index())
            }

            /// Construct from the underlying type without any checks.
            #[inline(always)]
            $v const fn from_raw_unchecked(raw: $raw) -> Self {
                Self { _raw: raw }
            }

            /// Get the wrapped index.
            #[inline(always)]
            $v const fn raw(self) -> $raw {
                self._raw
            }

            /// Asserts `v <= Self::MAX_INDEX` unless Self::CHECKS_MAX_INDEX is false.
            #[inline]
            $v fn check_index(v: usize) {
                if Self::CHECKS_MAX_INDEX && (v > Self::MAX_INDEX) {
                    $crate::vec::__max_check_fail(v, Self::MAX_INDEX);
                }
            }
        }

        impl core::fmt::Debug for $type {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, $dbg, self.index())
            }
        }

        impl core::cmp::PartialOrd<usize> for $type {
            #[inline]
            fn partial_cmp(&self, other: &usize) -> Option<core::cmp::Ordering> {
                self.index().partial_cmp(other)
            }
        }

        impl core::cmp::PartialOrd<$type> for usize {
            #[inline]
            fn partial_cmp(&self, other: &$type) -> Option<core::cmp::Ordering> {
                self.partial_cmp(&other.index())
            }
        }

        impl PartialEq<usize> for $type {
            #[inline]
            fn eq(&self, other: &usize) -> bool {
                self.index() == *other
            }
        }

        impl PartialEq<$type> for usize {
            #[inline]
            fn eq(&self, other: &$type) -> bool {
                *self == other.index()
            }
        }

        impl core::ops::Add<usize> for $type {
            type Output = Self;
            #[inline]
            fn add(self, other: usize) -> Self {
                // use wrapping add so that it's up to the index type whether or
                // not to check -- e.g. if checks are disabled, they're disabled
                // on both debug and release.
                Self::new(self.index().wrapping_add(other))
            }
        }

        impl core::ops::Sub<usize> for $type {
            type Output = Self;
            #[inline]
            fn sub(self, other: usize) -> Self {
                // use wrapping sub so that it's up to the index type whether or
                // not to check -- e.g. if checks are disabled, they're disabled
                // on both debug and release.
                Self::new(self.index().wrapping_sub(other))
            }
        }

        impl core::ops::AddAssign<usize> for $type {
            #[inline]
            fn add_assign(&mut self, other: usize) {
                *self = *self + other
            }
        }

        impl core::ops::SubAssign<usize> for $type {
            #[inline]
            fn sub_assign(&mut self, other: usize) {
                *self = *self - other;
            }
        }

        impl core::ops::Rem<usize> for $type {
            type Output = Self;
            #[inline]
            fn rem(self, other: usize) -> Self {
                Self::new(self.index() % other)
            }
        }

        impl core::ops::Add<$type> for usize {
            type Output = $type;
            #[inline]
            fn add(self, other: $type) -> $type {
                other + self
            }
        }

        impl core::ops::Sub<$type> for usize {
            type Output = $type;
            #[inline]
            fn sub(self, other: $type) -> $type {
                $type::new(self.wrapping_sub(other.index()))
            }
        }

        impl core::ops::Add for $type {
            type Output = $type;
            #[inline]
            fn add(self, other: $type) -> $type {
                $type::new(other.index() + self.index())
            }
        }

        impl core::ops::Sub for $type {
            type Output = $type;
            #[inline]
            fn sub(self, other: $type) -> $type {
                $type::new(self.index().wrapping_sub(other.index()))
            }
        }

        impl core::ops::AddAssign for $type {
            #[inline]
            fn add_assign(&mut self, other: $type) {
                *self = *self + other
            }
        }

        impl core::ops::SubAssign for $type {
            #[inline]
            fn sub_assign(&mut self, other: $type) {
                *self = *self - other;
            }
        }

        impl From<$type> for usize {
            #[inline]
            fn from(v: $type) -> usize {
                v.index()
            }
        }

        impl From<usize> for $type {
            #[inline]
            fn from(value: usize) -> Self {
                $type::from_usize(value)
            }
        }

        $crate::__internal_maybe_index_impl_serde!($type);
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __internal_impl_partialeq {
    ($Lhs: ty, $Rhs: ty) => {
        impl<'a, 'b, A, B, I: NonZeroIdx> PartialEq<$Rhs> for $Lhs
        where
            A: PartialEq<B>,
        {
            #[inline]
            fn eq(&self, other: &$Rhs) -> bool {
                self[..] == other[..]
            }
            #[inline]
            fn ne(&self, other: &$Rhs) -> bool {
                self[..] != other[..]
            }
        }
    };

    // TODO: REMOVE ME
    // with default
    (@ $Lhs: ty, $Rhs: ty) => {
        impl<'a, 'b, A: Default, B, I: NonZeroIdx> PartialEq<$Rhs> for $Lhs
        where
            A: PartialEq<B>,
        {
            #[inline]
            fn eq(&self, other: &$Rhs) -> bool {
                self[..] == other[..]
            }
            #[inline]
            fn ne(&self, other: &$Rhs) -> bool {
                self[..] != other[..]
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __internal_impl_partialeq2 {
    ($Lhs: ty, $Rhs: ty) => {
        impl<'a, 'b, A, B, I: NonZeroIdx, J: NonZeroIdx> PartialEq<$Rhs> for $Lhs
        where
            A: PartialEq<B>,
        {
            #[inline]
            fn eq(&self, other: &$Rhs) -> bool {
                self.raw[..] == other.raw[..]
            }
            #[inline]
            fn ne(&self, other: &$Rhs) -> bool {
                self.raw[..] != other.raw[..]
            }
        }
    };

    // TODO: REMOVE ME
    // with default A
    (@ $Lhs: ty, $Rhs: ty) => {
        $crate::__internal_impl_partialeq2!(@@ $Lhs, $Rhs);
    };

    // TODO: REMOVE ME
    // with default A and B
    (@@ $Lhs: ty, $Rhs: ty) => {
        impl<'a, 'b, A: Default, B: Default, I: NonZeroIdx, J: NonZeroIdx> PartialEq<$Rhs> for $Lhs
        where
            A: PartialEq<B>,
        {
            #[inline]
            fn eq(&self, other: &$Rhs) -> bool {
                self.raw[..] == other.raw[..]
            }
            #[inline]
            fn ne(&self, other: &$Rhs) -> bool {
                self.raw[..] != other.raw[..]
            }
        }
    };
}