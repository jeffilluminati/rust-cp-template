use std::{
    iter::{from_fn, repeat_with, FromIterator},
    marker::PhantomData,
};

pub fn read_stdin_all() -> String {
    use std::io::Read as _;
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).expect("io error");
    s
}
pub fn read_stdin_all_unchecked() -> String {
    use std::io::Read as _;
    let mut buf = Vec::new();
    std::io::stdin().read_to_end(&mut buf).expect("io error");
    unsafe { String::from_utf8_unchecked(buf) }
}
pub fn read_all(mut reader: impl std::io::Read) -> String {
    let mut s = String::new();
    reader.read_to_string(&mut s).expect("io error");
    s
}
pub fn read_all_unchecked(mut reader: impl std::io::Read) -> String {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).expect("io error");
    unsafe { String::from_utf8_unchecked(buf) }
}
pub fn read_stdin_line() -> String {
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).expect("io error");
    s
}
pub trait IterScan: Sized {
    type Output<'a>;
    fn scan<'a, I: Iterator<Item = &'a str>>(iter: &mut I) -> Option<Self::Output<'a>>;
}
pub trait MarkedIterScan: Sized {
    type Output<'a>;
    fn mscan<'a, I: Iterator<Item = &'a str>>(self, iter: &mut I) -> Option<Self::Output<'a>>;
}
#[derive(Clone, Debug)]
pub struct Scanner<'a, I: Iterator<Item = &'a str> = std::str::SplitAsciiWhitespace<'a>> {
    iter: I,
}
impl<'a> Scanner<'a> {
    pub fn new(s: &'a str) -> Self {
        let iter = s.split_ascii_whitespace();
        Self { iter }
    }
}
impl<'a, I: Iterator<Item = &'a str>> Scanner<'a, I> {
    pub fn new_from_iter(iter: I) -> Self {
        Self { iter }
    }
    pub fn scan<T>(&mut self) -> <T as IterScan>::Output<'a>
    where
        T: IterScan,
    {
        <T as IterScan>::scan(&mut self.iter).expect("scan error")
    }
    pub fn mscan<T>(&mut self, marker: T) -> <T as MarkedIterScan>::Output<'a>
    where
        T: MarkedIterScan,
    {
        marker.mscan(&mut self.iter).expect("scan error")
    }
    pub fn scan_vec<T>(&mut self, size: usize) -> Vec<<T as IterScan>::Output<'a>>
    where
        T: IterScan,
    {
        (0..size)
            .map(|_| <T as IterScan>::scan(&mut self.iter).expect("scan error"))
            .collect()
    }
    #[inline]
    pub fn iter<'b, T>(&'b mut self) -> ScannerIter<'a, 'b, I, T>
    where
        T: IterScan,
    {
        ScannerIter {
            inner: self,
            _marker: std::marker::PhantomData,
        }
    }
}

macro_rules! impl_iter_scan {
    ($($t:ty)*) => {$(
        impl IterScan for $t {
            type Output<'a> = Self;
            fn scan<'a, I: Iterator<Item = &'a str>>(iter: &mut I) -> Option<Self> {
                iter.next()?.parse::<$t>().ok()
            }
        })*
    };
}
impl_iter_scan!(char u8 u16 u32 u64 usize i8 i16 i32 i64 isize f32 f64 u128 i128 String);

macro_rules! impl_iter_scan_tuple {
    (@impl $($T:ident)*) => {
        impl<$($T: IterScan),*> IterScan for ($($T,)*) {
            type Output<'a> = ($(<$T as IterScan>::Output<'a>,)*);
            fn scan<'a, It: Iterator<Item = &'a str>>(_iter: &mut It) -> Option<Self::Output<'a>> {
                Some(($(<$T as IterScan>::scan(_iter)?,)*))
            }
        }
    };
    (@inner $($T:ident)*,) => {
        impl_iter_scan_tuple!(@impl $($T)*);
    };
    (@inner $($T:ident)*, $U:ident $($Rest:ident)*) => {
        impl_iter_scan_tuple!(@impl $($T)*);
        impl_iter_scan_tuple!(@inner $($T)* $U, $($Rest)*);
    };
    ($($T:ident)*) => {
        impl_iter_scan_tuple!(@inner , $($T)*);
    };
}
impl_iter_scan_tuple!(A B C D E F G H I J K);

pub struct ScannerIter<'a, 'b, I: Iterator<Item = &'a str>, T> {
    inner: &'b mut Scanner<'a, I>,
    _marker: std::marker::PhantomData<fn() -> T>,
}
impl<'a, I, T> Iterator for ScannerIter<'a, '_, I, T>
where
    I: Iterator<Item = &'a str>,
    T: IterScan,
{
    type Item = <T as IterScan>::Output<'a>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        <T as IterScan>::scan(&mut self.inner.iter)
    }
}

/// scan a value with Scanner
///
/// - `scan_value!(scanner, ELEMENT)`
///
/// ELEMENT :=
/// - `$ty`: IterScan
/// - `@$expr`: MarkedIterScan
/// - `$ty = $expr`: MarkedIterScan
/// - `[ELEMENT; $expr]`: vector
/// - `[ELEMENT; const $expr]`: array
/// - `[ELEMENT]`: iterator
/// - `($(ELEMENT)*,)`: tuple
#[macro_export]
macro_rules! scan_value {
    (@repeat $scanner:expr, [$($t:tt)*] $($len:expr)?)                           => { ::std::iter::repeat_with(|| $crate::scan_value!(@inner $scanner, [] $($t)*)) $(.take($len).collect::<Vec<_>>())? };
    (@array $scanner:expr, [$($t:tt)*] $len:expr)                                => { $crate::array![|| $crate::scan_value!(@inner $scanner, [] $($t)*); $len] };
    (@tuple $scanner:expr, [$([$($args:tt)*])*])                                 => { ($($($args)*,)*) };
    (@sparen $scanner:expr, [] @$e:expr; $($t:tt)*)                              => { $crate::scan_value!(@sparen $scanner, [@$e] $($t)*) };
    (@sparen $scanner:expr, [] ($($tt:tt)*); $($t:tt)*)                          => { $crate::scan_value!(@sparen $scanner, [($($tt)*)] $($t)*) };
    (@sparen $scanner:expr, [] [$($tt:tt)*]; $($t:tt)*)                          => { $crate::scan_value!(@sparen $scanner, [[$($tt)*]] $($t)*) };
    (@sparen $scanner:expr, [] $ty:ty = $e:expr; $($t:tt)*)                      => { $crate::scan_value!(@sparen $scanner, [$ty = $e] $($t)*) };
    (@sparen $scanner:expr, [] $ty:ty; $($t:tt)*)                                => { $crate::scan_value!(@sparen $scanner, [$ty] $($t)*) };
    (@sparen $scanner:expr, [] $($args:tt)*)                                     => { $crate::scan_value!(@repeat $scanner, [$($args)*]) };
    (@sparen $scanner:expr, [$($args:tt)+] const $len:expr)                      => { $crate::scan_value!(@array $scanner, [$($args)+] $len) };
    (@sparen $scanner:expr, [$($args:tt)+] $len:expr)                            => { $crate::scan_value!(@repeat $scanner, [$($args)+] $len) };
    (@$tag:ident $scanner:expr, [[$($args:tt)*]])                                => { $($args)* };
    (@$tag:ident $scanner:expr, [$($args:tt)*] @$e:expr $(, $($t:tt)*)?)         => { $crate::scan_value!(@$tag $scanner, [$($args)* [$scanner.mscan($e)]] $(, $($t)*)?) };
    (@$tag:ident $scanner:expr, [$($args:tt)*] ($($tuple:tt)*) $($t:tt)*)        => { $crate::scan_value!(@$tag $scanner, [$($args)* [$crate::scan_value!(@tuple $scanner, [] $($tuple)*)]] $($t)*) };
    (@$tag:ident $scanner:expr, [$($args:tt)*] [$($tt:tt)*] $($t:tt)*)           => { $crate::scan_value!(@$tag $scanner, [$($args)* [$crate::scan_value!(@sparen $scanner, [] $($tt)*)]] $($t)*) };
    (@$tag:ident $scanner:expr, [$($args:tt)*] $ty:ty = $e:expr $(, $($t:tt)*)?) => { $crate::scan_value!(@$tag $scanner, [$($args)* [{ let _tmp: $ty = $scanner.mscan($e); _tmp }]] $(, $($t)*)?) };
    (@$tag:ident $scanner:expr, [$($args:tt)*] $ty:ty $(, $($t:tt)*)?)           => { $crate::scan_value!(@$tag $scanner, [$($args)* [$scanner.scan::<$ty>()]] $(, $($t)*)?) };
    (@$tag:ident $scanner:expr, [$($args:tt)*] , $($t:tt)*)                      => { $crate::scan_value!(@$tag $scanner, [$($args)*] $($t)*) };
    (@$tag:ident $scanner:expr, [$($args:tt)*])                                  => { ::std::compile_error!(::std::stringify!($($args)*)) };
    (src = $src:expr, $($t:tt)*)                                                 => { { let mut __scanner = Scanner::new($src); $crate::scan_value!(@inner __scanner, [] $($t)*) } };
    (iter = $iter:expr, $($t:tt)*)                                               => { { let mut __scanner = Scanner::new_from_iter($iter); $crate::scan_value!(@inner __scanner, [] $($t)*) } };
    ($scanner:expr, $($t:tt)*)                                                   => { $crate::scan_value!(@inner $scanner, [] $($t)*) }
}

/// scan and bind values with Scanner
///
/// - `scan!(scanner, $($pat $(: ELEMENT)?),*)`
#[macro_export]
macro_rules! scan {
    (@assert $p:pat) => {};
    (@assert $($p:tt)*) => { ::std::compile_error!(::std::concat!("expected pattern, found `", ::std::stringify!($($p)*), "`")); };
    (@pat $scanner:expr, [] [])                                                     => {};
    (@pat $scanner:expr, [] [] , $($t:tt)*)                                         => { $crate::scan!(@pat $scanner, [] [] $($t)*) };
    (@pat $scanner:expr, [$($p:tt)*] [] $x:ident $($t:tt)*)                         => { $crate::scan!(@pat $scanner, [$($p)* $x] [] $($t)*) };
    (@pat $scanner:expr, [$($p:tt)*] [] :: $($t:tt)*)                               => { $crate::scan!(@pat $scanner, [$($p)* ::] [] $($t)*) };
    (@pat $scanner:expr, [$($p:tt)*] [] & $($t:tt)*)                                => { $crate::scan!(@pat $scanner, [$($p)* &] [] $($t)*) };
    (@pat $scanner:expr, [$($p:tt)*] [] ($($x:tt)*) $($t:tt)*)                      => { $crate::scan!(@pat $scanner, [$($p)* ($($x)*)] [] $($t)*) };
    (@pat $scanner:expr, [$($p:tt)*] [] [$($x:tt)*] $($t:tt)*)                      => { $crate::scan!(@pat $scanner, [$($p)* [$($x)*]] [] $($t)*) };
    (@pat $scanner:expr, [$($p:tt)*] [] {$($x:tt)*} $($t:tt)*)                      => { $crate::scan!(@pat $scanner, [$($p)* {$($x)*}] [] $($t)*) };
    (@pat $scanner:expr, [$($p:tt)*] [] : $($t:tt)*)                                => { $crate::scan!(@ty  $scanner, [$($p)*] [] $($t)*) };
    (@pat $scanner:expr, [$($p:tt)*] [] $($t:tt)*)                                  => { $crate::scan!(@let $scanner, [$($p)*] [usize] $($t)*) };
    (@ty  $scanner:expr, [$($p:tt)*] [$($tt:tt)*] @$e:expr $(, $($t:tt)*)?)         => { $crate::scan!(@let $scanner, [$($p)*] [$($tt)* @$e] $(, $($t)*)?) };
    (@ty  $scanner:expr, [$($p:tt)*] [$($tt:tt)*] ($($x:tt)*) $($t:tt)*)            => { $crate::scan!(@let $scanner, [$($p)*] [$($tt)* ($($x)*)] $($t)*) };
    (@ty  $scanner:expr, [$($p:tt)*] [$($tt:tt)*] [$($x:tt)*] $($t:tt)*)            => { $crate::scan!(@let $scanner, [$($p)*] [$($tt)* [$($x)*]] $($t)*) };
    (@ty  $scanner:expr, [$($p:tt)*] [$($tt:tt)*] $ty:ty = $e:expr $(, $($t:tt)*)?) => { $crate::scan!(@let $scanner, [$($p)*] [$($tt)* $ty = $e] $(, $($t)*)?) };
    (@ty  $scanner:expr, [$($p:tt)*] [$($tt:tt)*] $ty:ty $(, $($t:tt)*)?)           => { $crate::scan!(@let $scanner, [$($p)*] [$($tt)* $ty] $(, $($t)*)?) };
    (@let $scanner:expr, [$($p:tt)*] [$($tt:tt)*] $($t:tt)*) => {
        $crate::scan!{@assert $($p)*}
        let $($p)* = $crate::scan_value!($scanner, $($tt)*);
        $crate::scan!(@pat $scanner, [] [] $($t)*)
    };
    (src = $src:expr, $($t:tt)*)   => { let mut __scanner = Scanner::new($src); $crate::scan!(@pat __scanner, [] [] $($t)*) };
    (iter = $iter:expr, $($t:tt)*) => { let mut __scanner = Scanner::new_from_iter($iter); $crate::scan!(@pat __scanner, [] [] $($t)*) };
    ($scanner:expr, $($t:tt)*) => { $crate::scan!(@pat $scanner, [] [] $($t)*) }
}

/// define enum scan rules
///
/// # Example
/// ```rust
/// # use competitive::{define_enum_scan, tools::{CharsWithBase, IterScan, Scanner, Usize1}};
/// define_enum_scan! {
///   enum Query: u8 {
///     0 => Noop,
///     1 => Args { i: Usize1, s: char },
///     9 => Complex { n: usize, c: [(usize, Vec<usize> = CharsWithBase('a')); n] },
///   }
/// }
/// ```
#[macro_export]
macro_rules! define_enum_scan {
    (@field_ty $lt:lifetime @repeat [$($t:tt)*] $($len:expr)?)                           => { Vec<$crate::define_enum_scan!(@field_ty $lt $($t)*)> };
    (@field_ty $lt:lifetime @array [$($t:tt)*] $len:expr)                                => { [$crate::define_enum_scan!(@field_ty $lt $($t)*); $len] };
    (@field_ty $lt:lifetime @tuple [$([$($args:tt)*])*])                                 => { ($( $($args)* ,)*) };
    (@field_ty $lt:lifetime @sparen [] ($($tt:tt)*); $($t:tt)*)                          => { $crate::define_enum_scan!(@field_ty $lt @sparen [($($tt)*)] $($t)*) };
    (@field_ty $lt:lifetime @sparen [] [$($tt:tt)*]; $($t:tt)*)                          => { $crate::define_enum_scan!(@field_ty $lt @sparen [[$($tt)*]] $($t)*) };
    (@field_ty $lt:lifetime @sparen [] $ty:ty = $e:expr; $($t:tt)*)                      => { $crate::define_enum_scan!(@field_ty $lt @sparen [$ty = $e] $($t)*) };
    (@field_ty $lt:lifetime @sparen [] $ty:ty; $($t:tt)*)                                => { $crate::define_enum_scan!(@field_ty $lt @sparen [$ty] $($t)*) };
    (@field_ty $lt:lifetime @sparen [] $($args:tt)*)                                     => { $crate::define_enum_scan!(@field_ty $lt @repeat [$($args)*]) };
    (@field_ty $lt:lifetime @sparen [$($args:tt)+] const $len:expr)                      => { $crate::define_enum_scan!(@field_ty $lt @array [$($args)+] $len) };
    (@field_ty $lt:lifetime @sparen [$($args:tt)+] $len:expr)                            => { $crate::define_enum_scan!(@field_ty $lt @repeat [$($args)+] $len) };
    (@field_ty $lt:lifetime @$tag:ident [$($args:tt)*] ($($tuple:tt)*) $($t:tt)*)        => { $crate::define_enum_scan!(@field_ty $lt @$tag [$($args)* [$crate::define_enum_scan!(@field_ty $lt @tuple [] $($tuple)*)]] $($t)*) };
    (@field_ty $lt:lifetime @$tag:ident [$($args:tt)*] [$($tt:tt)*] $($t:tt)*)           => { $crate::define_enum_scan!(@field_ty $lt @$tag [$($args)* [$crate::define_enum_scan!(@field_ty $lt @sparen [] $($tt)*)]] $($t)*) };
    (@field_ty $lt:lifetime @$tag:ident [$($args:tt)*] $ty:ty = $e:expr $(, $($t:tt)*)?) => { $crate::define_enum_scan!(@field_ty $lt @$tag [$($args)* [$ty]] $(, $($t)*)?) };
    (@field_ty $lt:lifetime @$tag:ident [$($args:tt)*] $ty:ty $(, $($t:tt)*)?)           => { $crate::define_enum_scan!(@field_ty $lt @$tag [$($args)* [<$ty as IterScan>::Output<$lt>]] $(, $($t)*)?) };
    (@field_ty $lt:lifetime @$tag:ident [$($args:tt)*] , $($t:tt)*)                      => { $crate::define_enum_scan!(@field_ty $lt @$tag [$($args)*] $($t)*) };
    (@field_ty $lt:lifetime @$tag:ident [[$($args:tt)*]])                                => { $($args)* };
    (@field_ty $lt:lifetime @$tag:ident [$($args:tt)*])                                  => { ::std::compile_error!(::std::stringify!($($args)*)) };
    (@field_ty $lt:lifetime $($t:tt)*) => { $crate::define_enum_scan!(@field_ty $lt @inner [] $($t)*) };

    (@tag_expr raw, $iter:ident) => { $iter.next()? };
    (@tag_expr $d:ty, $iter:ident) => { <$d as IterScan>::scan($iter)? };
    (@variant ([$($attr:tt)*] $vis:vis $T:ident $d:tt) [$($vars:tt)*]) => { $crate::define_enum_scan! { @def $($attr)* $vis enum $T : $d { $($vars)* } } };
    (@variant $ctx:tt [$($vars:tt)*] $p:pat => $v:ident { $($fs:tt)* } $($rest:tt)*) => { $crate::define_enum_scan! { @field   $ctx [$($vars)*] $p => $v [] $($fs)* ; $($rest)* } };
    (@variant $ctx:tt [$($vars:tt)*] $p:pat => $v:ident $($rest:tt)*)                    => { $crate::define_enum_scan! { @variant $ctx [$($vars)* $p => $v ,] $($rest)* } };
    (@variant $ctx:tt [$($vars:tt)*] , $($rest:tt)*)                                     => { $crate::define_enum_scan! { @variant $ctx [$($vars)*] $($rest)* } };
    (@endfield $ctx:tt [$($vars:tt)*] $p:pat => $v:ident [$($fs:tt)*] [$f:ident : $($spec:tt)*] , $($rest:tt)*) => { $crate::define_enum_scan! { @field $ctx [$($vars)*] $p => $v [$($fs)* [$f : $($spec)*]] $($rest)* } };
    (@endfield $ctx:tt [$($vars:tt)*] $p:pat => $v:ident [$($fs:tt)*] [$f:ident : $($spec:tt)*] ; $($rest:tt)*) => { $crate::define_enum_scan! { @variant $ctx [$($vars)* $p => $v { $($fs)* [$f : $($spec)*] } ,] $($rest)* } };
    (@field $ctx:tt [$($vars:tt)*] $p:pat => $v:ident [$($fs:tt)*] ; $($rest:tt)*)                                  => { $crate::define_enum_scan! { @variant $ctx [$($vars)* $p => $v { $($fs)* } ,] $($rest)* } };
    (@field $ctx:tt [$($vars:tt)*] $p:pat => $v:ident [$($fs:tt)*] $f:ident : ($($tuple:tt)*) $sep:tt $($rest:tt)*) => { $crate::define_enum_scan! { @endfield $ctx [$($vars)*] $p => $v [$($fs)*] [$f : ($($tuple)*)] $sep $($rest)* } };
    (@field $ctx:tt [$($vars:tt)*] $p:pat => $v:ident [$($fs:tt)*] $f:ident : [$($x:tt)*] $sep:tt $($rest:tt)*)     => { $crate::define_enum_scan! { @endfield $ctx [$($vars)*] $p => $v [$($fs)*] [$f : [$($x)*]] $sep $($rest)* } };
    (@field $ctx:tt [$($vars:tt)*] $p:pat => $v:ident [$($fs:tt)*] $f:ident : $ty:ty = $e:expr , $($rest:tt)*)      => { $crate::define_enum_scan! { @endfield $ctx [$($vars)*] $p => $v [$($fs)*] [$f : $ty = $e] , $($rest)* } };
    (@field $ctx:tt [$($vars:tt)*] $p:pat => $v:ident [$($fs:tt)*] $f:ident : $ty:ty ; $($rest:tt)*)                => { $crate::define_enum_scan! { @endfield $ctx [$($vars)*] $p => $v [$($fs)*] [$f : $ty] ; $($rest)* } };
    (@field $ctx:tt [$($vars:tt)*] $p:pat => $v:ident [$($fs:tt)*] $f:ident : $ty:ty = $e:expr ; $($rest:tt)*)      => { $crate::define_enum_scan! { @endfield $ctx [$($vars)*] $p => $v [$($fs)*] [$f : $ty = $e] ; $($rest)* } };
    (@field $ctx:tt [$($vars:tt)*] $p:pat => $v:ident [$($fs:tt)*] $f:ident : $ty:ty , $($rest:tt)*)                => { $crate::define_enum_scan! { @endfield $ctx [$($vars)*] $p => $v [$($fs)*] [$f : $ty] , $($rest)* } };
    (
        @def
        $(#[$attr:meta])*
        $vis:vis enum $T:ident : $d:tt {
            $( $p:pat => $v:ident $( { $( [$f:ident : $($spec:tt)*] )* } )?, )*
        }
    ) => {
        $(#[$attr])*
        $vis enum $T<'__scan> {
            $( $v $( { $( $f : $crate::define_enum_scan!(@field_ty '__scan $($spec)*) ),* } )? ),*,
            #[doc(hidden)]
            __Lifetime(&'__scan ::std::convert::Infallible),
        }
        impl<'__scan> IterScan for $T<'__scan> {
            type Output<'a> = $T<'a>;
            fn scan<'a, I: Iterator<Item = &'a str>>(iter: &mut I) -> Option<Self::Output<'a>> {
                let tag = $crate::define_enum_scan!(@tag_expr $d, iter);
                match tag {
                    $(
                        $p => {
                            $($(
                                let $f = $crate::scan_value!(iter = &mut *iter, $($spec)* );
                            )*)?
                            Some($T::$v $( { $( $f ),* } )?)
                        }
                    ),*
                    _ => None,
                }
            }
        }
    };
    (
        $(#[$attr:meta])*
        $vis:vis enum $T:ident : raw {
            $($body:tt)*
        }
    ) => {
        $crate::define_enum_scan! { @variant ([$(#[$attr])*] $vis $T raw) [] $($body)* }
    };
    (
        $(#[$attr:meta])*
        $vis:vis enum $T:ident : $d:ty {
            $($body:tt)*
        }
    ) => {
        $crate::define_enum_scan! { @variant ([$(#[$attr])*] $vis $T $d) [] $($body)* }
    };
}

#[derive(Debug, Copy, Clone)]
pub enum Usize1 {}
impl IterScan for Usize1 {
    type Output<'a> = usize;
    fn scan<'a, I: Iterator<Item = &'a str>>(iter: &mut I) -> Option<Self::Output<'a>> {
        <usize as IterScan>::scan(iter)?.checked_sub(1)
    }
}
#[derive(Debug, Copy, Clone)]
pub struct CharWithBase(pub char);
impl MarkedIterScan for CharWithBase {
    type Output<'a> = usize;
    fn mscan<'a, I: Iterator<Item = &'a str>>(self, iter: &mut I) -> Option<Self::Output<'a>> {
        Some((<char as IterScan>::scan(iter)? as u8 - self.0 as u8) as usize)
    }
}
#[derive(Debug, Copy, Clone)]
pub enum Chars {}
impl IterScan for Chars {
    type Output<'a> = Vec<char>;
    fn scan<'a, I: Iterator<Item = &'a str>>(iter: &mut I) -> Option<Self::Output<'a>> {
        Some(iter.next()?.chars().collect())
    }
}
#[derive(Debug, Copy, Clone)]
pub struct CharsWithBase(pub char);
impl MarkedIterScan for CharsWithBase {
    type Output<'a> = Vec<usize>;
    fn mscan<'a, I: Iterator<Item = &'a str>>(self, iter: &mut I) -> Option<Self::Output<'a>> {
        Some(
            iter.next()?
                .chars()
                .map(|c| (c as u8 - self.0 as u8) as usize)
                .collect(),
        )
    }
}
#[derive(Debug, Copy, Clone)]
pub enum Byte1 {}
impl IterScan for Byte1 {
    type Output<'a> = u8;
    fn scan<'a, I: Iterator<Item = &'a str>>(iter: &mut I) -> Option<Self::Output<'a>> {
        let bytes = iter.next()?.as_bytes();
        assert_eq!(bytes.len(), 1);
        Some(bytes[0])
    }
}
#[derive(Debug, Copy, Clone)]
pub struct ByteWithBase(pub u8);
impl MarkedIterScan for ByteWithBase {
    type Output<'a> = usize;
    fn mscan<'a, I: Iterator<Item = &'a str>>(self, iter: &mut I) -> Option<Self::Output<'a>> {
        Some((<char as IterScan>::scan(iter)? as u8 - self.0) as usize)
    }
}
#[derive(Debug, Copy, Clone)]
pub enum Bytes {}
impl IterScan for Bytes {
    type Output<'a> = &'a [u8];
    fn scan<'a, I: Iterator<Item = &'a str>>(iter: &mut I) -> Option<Self::Output<'a>> {
        Some(iter.next()?.as_bytes())
    }
}
#[derive(Debug, Copy, Clone)]
pub struct BytesWithBase(pub u8);
impl MarkedIterScan for BytesWithBase {
    type Output<'a> = Vec<usize>;
    fn mscan<'a, I: Iterator<Item = &'a str>>(self, iter: &mut I) -> Option<Self::Output<'a>> {
        Some(
            iter.next()?
                .bytes()
                .map(|c| (c - self.0) as usize)
                .collect(),
        )
    }
}
#[derive(Debug, Copy, Clone)]
pub enum VecScanCollect {}
pub trait IterScanCollect<T: IterScan> {
    type Output<'a>: FromIterator<<T as IterScan>::Output<'a>>;
}
impl<T> IterScanCollect<T> for VecScanCollect
where
    T: IterScan,
{
    type Output<'a> = Vec<<T as IterScan>::Output<'a>>;
}
#[derive(Debug, Copy, Clone)]
pub struct FromIteratorScanCollect<B>(PhantomData<fn() -> B>);
impl<T, B> IterScanCollect<T> for FromIteratorScanCollect<B>
where
    T: IterScan,
    for<'a> B: FromIterator<<T as IterScan>::Output<'a>>,
{
    type Output<'a> = B;
}
#[derive(Debug, Copy, Clone)]
pub struct Collect<T, B = VecScanCollect>
where
    T: IterScan,
    B: IterScanCollect<T>,
{
    size: usize,
    _marker: PhantomData<fn() -> (T, B)>,
}
impl<T, B> Collect<T, B>
where
    T: IterScan,
    B: IterScanCollect<T>,
{
    pub fn new(size: usize) -> Self {
        Self {
            size,
            _marker: PhantomData,
        }
    }
}
impl<T, B> MarkedIterScan for Collect<T, B>
where
    T: IterScan,
    B: IterScanCollect<T>,
{
    type Output<'a> = <B as IterScanCollect<T>>::Output<'a>;
    fn mscan<'a, I: Iterator<Item = &'a str>>(self, iter: &mut I) -> Option<Self::Output<'a>> {
        repeat_with(|| <T as IterScan>::scan(iter))
            .take(self.size)
            .collect()
    }
}
#[derive(Debug, Copy, Clone)]
pub struct SizedCollect<T, B = VecScanCollect>
where
    T: IterScan,
    B: IterScanCollect<T>,
{
    _marker: PhantomData<fn() -> (T, B)>,
}
impl<T, B> IterScan for SizedCollect<T, B>
where
    T: IterScan,
    B: IterScanCollect<T>,
{
    type Output<'a> = <B as IterScanCollect<T>>::Output<'a>;
    fn scan<'a, I: Iterator<Item = &'a str>>(iter: &mut I) -> Option<Self::Output<'a>> {
        let size = usize::scan(iter)?;
        repeat_with(|| <T as IterScan>::scan(iter))
            .take(size)
            .collect()
    }
}
#[derive(Debug, Copy, Clone)]
pub struct Splitted<T, P>
where
    T: IterScan,
{
    pat: P,
    _marker: PhantomData<fn() -> T>,
}
impl<T, P> Splitted<T, P>
where
    T: IterScan,
{
    pub fn new(pat: P) -> Self {
        Self {
            pat,
            _marker: PhantomData,
        }
    }
}
impl<T> MarkedIterScan for Splitted<T, char>
where
    T: IterScan,
{
    type Output<'a> = Vec<<T as IterScan>::Output<'a>>;
    fn mscan<'a, I: Iterator<Item = &'a str>>(self, iter: &mut I) -> Option<Self::Output<'a>> {
        let mut iter = iter.next()?.split(self.pat);
        Some(from_fn(|| <T as IterScan>::scan(&mut iter)).collect())
    }
}
impl<T> MarkedIterScan for Splitted<T, &str>
where
    T: IterScan,
{
    type Output<'a> = Vec<<T as IterScan>::Output<'a>>;

    fn mscan<'a, I: Iterator<Item = &'a str>>(self, iter: &mut I) -> Option<Self::Output<'a>> {
        let mut iter = iter.next()?.split(self.pat);
        Some(from_fn(|| <T as IterScan>::scan(&mut iter)).collect())
    }
}
impl<T, F> MarkedIterScan for F
where
    F: Fn(&str) -> Option<T>,
{
    type Output<'a> = T;
    fn mscan<'a, I: Iterator<Item = &'a str>>(self, iter: &mut I) -> Option<Self::Output<'a>> {
        self(iter.next()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan() {
        let mut s = Scanner::new("1 2 3 a 1 2 1 1 1.1 2 3");
        scan!(s, x, y: char, z: Usize1, a: @CharWithBase('a'), b: [usize; 2], c: (usize, @CharWithBase('0')), d: @Splitted::<usize, _>::new('.'), e: [usize; const 2]);
        assert_eq!(x, 1);
        assert_eq!(y, '2');
        assert_eq!(z, 2);
        assert_eq!(a, 0);
        assert_eq!(b, vec![1, 2]);
        assert_eq!(c, (1, 1));
        assert_eq!(d, vec![1, 1]);
        assert_eq!(e, [2, 3]);

        scan!(src = "12 34", c: Vec<usize> = CharsWithBase('0'), d: [Vec<usize> = CharsWithBase('0'); 1]);
        assert_eq!(c, vec![1, 2]);
        assert_eq!(d, vec![vec![3, 4]]);

        scan!(src = "1", x);
        assert_eq!(x, 1);
        assert_eq!(scan_value!(src = "1", usize), 1);

        scan!(iter = "1".split_ascii_whitespace(), x);
        assert_eq!(x, 1);
        assert_eq!(scan_value!(iter = "1".split_ascii_whitespace(), usize), 1);
    }

    #[test]
    fn test_zero_copy_bytes_abstractions() {
        let src = "ab cd ef";
        let mut s = Scanner::new(src);
        let bytes: Vec<&[u8]> = s.mscan(Collect::<Bytes>::new(3));
        assert_eq!(
            bytes,
            vec![b"ab".as_slice(), b"cd".as_slice(), b"ef".as_slice()]
        );
        assert_eq!(bytes[0].as_ptr(), src.as_ptr());

        let mut s = Scanner::new("2 ab cd");
        let bytes: Vec<&[u8]> = s.scan::<SizedCollect<Bytes>>();
        assert_eq!(bytes, vec![b"ab".as_slice(), b"cd".as_slice()]);

        let src = "ab,cd";
        let mut s = Scanner::new(src);
        let bytes: Vec<&[u8]> = s.mscan(Splitted::<Bytes, _>::new(','));
        assert_eq!(bytes, vec![b"ab".as_slice(), b"cd".as_slice()]);
        assert_eq!(bytes[0].as_ptr(), src.as_ptr());
    }

    #[test]
    fn test_define_enum_scan() {
        define_enum_scan! {
            enum Query: u8 {
                0 => Noop,
                1 => Args { i: Usize1, s: char },
                9 => Complex { n: usize, c: [(usize, Vec<usize> = CharsWithBase('a')); n] },
            }
        }
        define_enum_scan! {
            enum BorrowQuery: u8 {
                0 => Data { s: Bytes },
            }
        }

        let mut s = Scanner::new("0   1 2 a  9 2 3 ab 2 ab");
        scan!(s, q1: Query, q2: Query, q3: Query);
        match q1 {
            Query::Noop => {}
            _ => panic!("unexpected"),
        }
        match q2 {
            Query::Args { i, s } => {
                assert_eq!(i, 1);
                assert_eq!(s, 'a');
            }
            _ => panic!("unexpected"),
        }
        match q3 {
            Query::Complex { n, c } => {
                assert_eq!(n, 2);
                assert_eq!(c, vec![(3, vec![0, 1]), (2, vec![0, 1])]);
            }
            _ => panic!("unexpected"),
        }

        let src = "0 ab";
        let mut s = Scanner::new(src);
        let q = s.scan::<BorrowQuery>();
        match q {
            BorrowQuery::Data { s } => {
                assert_eq!(s, b"ab");
                assert_eq!(s.as_ptr(), unsafe { src.as_ptr().add(2) });
            }
            _ => panic!("unexpected"),
        }
    }
}
