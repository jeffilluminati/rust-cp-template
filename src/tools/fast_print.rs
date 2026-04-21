use std::{fmt::Display, io::Write};

#[doc(hidden)]
pub struct __FastPrintValue<T>(pub T);

#[doc(hidden)]
pub trait __FastPrintValueDispatch<W: Write> {
    fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>);
}

#[doc(hidden)]
pub struct __FastPrintNoSepIter<I>(pub Option<I>);

#[doc(hidden)]
pub trait __FastPrintNoSepDispatch<W: Write> {
    fn __fast_print_nosep(self, writer: &mut crate::tools::FastOutput<W>);
}

impl<I, W> __FastPrintNoSepDispatch<W> for &mut __FastPrintNoSepIter<I>
where
    I: IntoIterator,
    I::Item: Display,
    W: Write,
{
    fn __fast_print_nosep(self, writer: &mut crate::tools::FastOutput<W>) {
        use std::io::Write as _;
        for item in self.0.take().expect("iterator consumed").into_iter() {
            ::std::write!(writer, "{}", item).expect("io error");
        }
    }
}

impl<I, W> __FastPrintNoSepDispatch<W> for &mut &mut __FastPrintNoSepIter<I>
where
    I: IntoIterator<Item = char>,
    W: Write,
{
    fn __fast_print_nosep(self, writer: &mut crate::tools::FastOutput<W>) {
        let s = self
            .0
            .take()
            .expect("iterator consumed")
            .into_iter()
            .collect::<String>();
        writer.bytes(s.as_bytes());
    }
}

impl<T, W> __FastPrintValueDispatch<W> for &__FastPrintValue<T>
where
    T: Display,
    W: Write,
{
    fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
        ::std::write!(writer, "{}", &self.0).expect("io error");
    }
}

macro_rules! impl_fast_print_unsigned {
    ($($t:ty => $method:ident),* $(,)?) => {
        $(
            impl<W> __FastPrintValueDispatch<W> for &&__FastPrintValue<$t>
            where
                W: Write,
            {
                fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
                    writer.$method(self.0);
                }
            }

            impl<W> __FastPrintValueDispatch<W> for &&__FastPrintValue<&$t>
            where
                W: Write,
            {
                fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
                    writer.$method(*self.0);
                }
            }
        )*
    };
}

macro_rules! impl_fast_print_signed {
    ($($t:ty => $method:ident),* $(,)?) => {
        $(
            impl<W> __FastPrintValueDispatch<W> for &&__FastPrintValue<$t>
            where
                W: Write,
            {
                fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
                    writer.$method(self.0);
                }
            }

            impl<W> __FastPrintValueDispatch<W> for &&__FastPrintValue<&$t>
            where
                W: Write,
            {
                fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
                    writer.$method(*self.0);
                }
            }
        )*
    };
}

impl_fast_print_unsigned!(u8 => u8, u16 => u16, u32 => u32, u64 => u64);
impl_fast_print_signed!(i8 => i8, i16 => i16, i32 => i32, i64 => i64);

impl<W> __FastPrintValueDispatch<W> for &&__FastPrintValue<usize>
where
    W: Write,
{
    fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
        writer.u64(self.0 as u64);
    }
}

impl<W> __FastPrintValueDispatch<W> for &&__FastPrintValue<&usize>
where
    W: Write,
{
    fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
        writer.u64(*self.0 as u64);
    }
}

impl<W> __FastPrintValueDispatch<W> for &&__FastPrintValue<isize>
where
    W: Write,
{
    fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
        writer.i64(self.0 as i64);
    }
}

impl<W> __FastPrintValueDispatch<W> for &&__FastPrintValue<&isize>
where
    W: Write,
{
    fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
        writer.i64(*self.0 as i64);
    }
}

impl<W> __FastPrintValueDispatch<W> for &&__FastPrintValue<char>
where
    W: Write,
{
    fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
        let mut buf = [0; 4];
        writer.bytes(self.0.encode_utf8(&mut buf).as_bytes());
    }
}

impl<W> __FastPrintValueDispatch<W> for &&__FastPrintValue<&char>
where
    W: Write,
{
    fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
        let mut buf = [0; 4];
        writer.bytes(self.0.encode_utf8(&mut buf).as_bytes());
    }
}

impl<W> __FastPrintValueDispatch<W> for &&__FastPrintValue<&str>
where
    W: Write,
{
    fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
        writer.bytes(self.0.as_bytes());
    }
}

impl<W> __FastPrintValueDispatch<W> for &&__FastPrintValue<String>
where
    W: Write,
{
    fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
        writer.bytes(self.0.as_bytes());
    }
}

impl<W> __FastPrintValueDispatch<W> for &&__FastPrintValue<&String>
where
    W: Write,
{
    fn __fast_print_value(self, writer: &mut crate::tools::FastOutput<W>) {
        writer.bytes(self.0.as_bytes());
    }
}

/// Print expressions with a [`FastOutput`](crate::tools::FastOutput).
/// This mirrors [`iter_print!`](crate::iter_print) but specializes common
/// primitive values for faster output.
#[macro_export]
macro_rules! fast_print {
    (@@value $writer:expr, $e:expr) => {{
        use $crate::tools::__FastPrintValueDispatch as _;
        let value = $crate::tools::__FastPrintValue($e);
        (&&value).__fast_print_value($writer);
    }};
    (@@fmt $writer:expr, $sep:expr, $is_head:expr, ($lit:literal $(, $e:expr)* $(,)?)) => {
        use ::std::io::Write as _;
        if !$is_head {
            $crate::fast_print!(@@value $writer, $sep);
        }
        ::std::write!($writer, $lit, $($e),*).expect("io error");
    };
    (@@item $writer:expr, $sep:expr, $is_head:expr, $e:expr) => {
        if !$is_head {
            $crate::fast_print!(@@value $writer, $sep);
        }
        $crate::fast_print!(@@value $writer, $e);
    };
    (@@bytes $writer:expr, $sep:expr, $is_head:expr, $bytes:expr) => {{
        if !$is_head {
            $crate::fast_print!(@@value $writer, $sep);
        }
        $writer.bytes(($bytes).as_ref());
    }};
    (@@b $writer:expr, $sep:expr, $is_head:expr, $bytes:expr) => {
        $crate::fast_print!(@@bytes $writer, $sep, $is_head, $bytes);
    };
    (@@line_feed $writer:expr $(,)?) => {
        $writer.byte(b'\n');
    };
    (@@it $writer:expr, $sep:expr, $is_head:expr, $iter:expr) => {{
        let mut iter = $iter.into_iter();
        if let Some(item) = iter.next() {
            $crate::fast_print!(@@item $writer, $sep, $is_head, item);
        }
        for item in iter {
            $crate::fast_print!(@@item $writer, $sep, false, item);
        }
    }};
    (@@it_nosep $writer:expr, $iter:expr) => {{
        use $crate::tools::__FastPrintNoSepDispatch as _;
        let mut iter = $crate::tools::__FastPrintNoSepIter(Some($iter));
        (&mut &mut iter).__fast_print_nosep($writer);
    }};
    (@@it1 $writer:expr, $sep:expr, $is_head:expr, $iter:expr) => {{
        let mut iter = $iter.into_iter();
        if let Some(item) = iter.next() {
            $crate::fast_print!(@@item $writer, $sep, $is_head, item + 1);
        }
        for item in iter {
            $crate::fast_print!(@@item $writer, $sep, false, item + 1);
        }
    }};
    (@@cw $writer:expr, $sep:expr, $is_head:expr, ($ch:literal $iter:expr)) => {{
        let mut iter = $iter.into_iter();
        let b = $ch as u8;
        if let Some(item) = iter.next() {
            $crate::fast_print!(@@item $writer, $sep, $is_head, (item as u8 + b) as char);
        }
        for item in iter {
            $crate::fast_print!(@@item $writer, $sep, false, (item as u8 + b) as char);
        }
    }};
    (@@bw $writer:expr, $sep:expr, $is_head:expr, ($b:literal $iter:expr)) => {{
        let mut iter = $iter.into_iter();
        let b: u8 = $b;
        if let Some(item) = iter.next() {
            $crate::fast_print!(@@item $writer, $sep, $is_head, (item as u8 + b) as char);
        }
        for item in iter {
            $crate::fast_print!(@@item $writer, $sep, false, (item as u8 + b) as char);
        }
    }};
    (@@it2d $writer:expr, $sep:expr, $is_head:expr, $iter:expr) => {
        let mut iter = $iter.into_iter();
        if let Some(item) = iter.next() {
            $crate::fast_print!(@@it $writer, $sep, $is_head, item);
        }
        for item in iter {
            $crate::fast_print!(@@line_feed $writer);
            $crate::fast_print!(@@it $writer, $sep, true, item);
        }
    };
    (@@tup $writer:expr, $sep:expr, $is_head:expr, $tuple:expr) => {
        $crate::tools::IterPrint::iter_print($tuple, &mut $writer, $sep, $is_head).expect("io error");
    };
    (@@ittup $writer:expr, $sep:expr, $is_head:expr, $iter:expr) => {
        let mut iter = $iter.into_iter();
        if let Some(item) = iter.next() {
            $crate::fast_print!(@@tup $writer, $sep, $is_head, item);
        }
        for item in iter {
            $crate::fast_print!(@@line_feed $writer);
            $crate::fast_print!(@@tup $writer, $sep, true, item);
        }
    };
    (@@assert_tag item) => {};
    (@@assert_tag it) => {};
    (@@assert_tag it1) => {};
    (@@assert_tag bytes) => {};
    (@@assert_tag b) => {};
    (@@assert_tag it2d) => {};
    (@@assert_tag tup) => {};
    (@@assert_tag ittup) => {};
    (@@assert_tag $tag:ident) => {
        ::std::compile_error!(::std::concat!("invalid tag in `fast_print!`: `", std::stringify!($tag), "`"));
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @sep $e:expr, $($t:tt)*) => {
        $crate::fast_print!(@@inner $writer, $e, $is_head, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @ns @it $e:expr, $($t:tt)*) => {
        $crate::fast_print!(@@it_nosep $writer, $e);
        $crate::fast_print!(@@inner $writer, "", false, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @ns @it $e:expr; $($t:tt)*) => {
        $crate::fast_print!(@@it_nosep $writer, $e);
        $crate::fast_print!(@@line_feed $writer);
        $crate::fast_print!(@@inner $writer, "", true, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @ns @it $e:expr) => {
        $crate::fast_print!(@@it_nosep $writer, $e);
        $crate::fast_print!(@@inner $writer, "", false,);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @ns $($t:tt)*) => {
        $crate::fast_print!(@@inner $writer, "", $is_head, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @lf $($t:tt)*) => {
        $crate::fast_print!(@@inner $writer, '\n', $is_head, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @sp $($t:tt)*) => {
        $crate::fast_print!(@@inner $writer, ' ', $is_head, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @flush $($t:tt)*) => {
        $writer.flush();
        $crate::fast_print!(@@inner $writer, $sep, $is_head, ! $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @fmt $arg:tt $($t:tt)*) => {
        $crate::fast_print!(@@fmt $writer, $sep, $is_head, $arg);
        $crate::fast_print!(@@inner $writer, $sep, $is_head, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @cw $arg:tt $($t:tt)*) => {
        $crate::fast_print!(@@cw $writer, $sep, $is_head, $arg);
        $crate::fast_print!(@@inner $writer, $sep, $is_head, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @bw $arg:tt $($t:tt)*) => {
        $crate::fast_print!(@@bw $writer, $sep, $is_head, $arg);
        $crate::fast_print!(@@inner $writer, $sep, $is_head, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @$tag:ident $e:expr, $($t:tt)*) => {
        $crate::fast_print!(@@assert_tag $tag);
        $crate::fast_print!(@@$tag $writer, $sep, $is_head, $e);
        $crate::fast_print!(@@inner $writer, $sep, false, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @$tag:ident $e:expr; $($t:tt)*) => {
        $crate::fast_print!(@@assert_tag $tag);
        $crate::fast_print!(@@$tag $writer, $sep, $is_head, $e);
        $crate::fast_print!(@@line_feed $writer);
        $crate::fast_print!(@@inner $writer, $sep, true, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @$tag:ident $e:expr) => {
        $crate::fast_print!(@@assert_tag $tag);
        $crate::fast_print!(@@$tag $writer, $sep, $is_head, $e);
        $crate::fast_print!(@@inner $writer, $sep, false,);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, @$tag:ident $($t:tt)*) => {
        ::std::compile_error!(::std::concat!("invalid expr in `fast_print!`: `", std::stringify!($($t)*), "`"));
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, , $($t:tt)*) => {
        $crate::fast_print!(@@inner $writer, $sep, $is_head, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, ; $($t:tt)*) => {
        $crate::fast_print!(@@line_feed $writer);
        $crate::fast_print!(@@inner $writer, $sep, $is_head, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, ! $(,)?) => {};
    (@@inner $writer:expr, $sep:expr, $is_head:expr, ! $($t:tt)*) => {
        $crate::fast_print!(@@inner $writer, $sep, $is_head, $($t)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr,) => {
        $crate::fast_print!(@@line_feed $writer);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, { $($t:tt)* } $($rest:tt)*) => {
        $crate::fast_print!(@@inner $writer, $sep, $is_head, $($t)*, !);
        $crate::fast_print!(@@inner $writer, $sep, $is_head, $($rest)*);
    };
    (@@inner $writer:expr, $sep:expr, $is_head:expr, $($t:tt)*) => {
        $crate::fast_print!(@@inner $writer, $sep, $is_head, @item $($t)*);
    };
    ($writer:expr, $($t:tt)*) => {{
        $crate::fast_print!(@@inner $writer, ' ', true, $($t)*);
    }};
}

#[cfg(test)]
mod tests {
    use crate::tools::FastOutput;

    #[test]
    fn test_fast_print() {
        let mut buf = Vec::new();
        {
            let mut out = FastOutput::new(&mut buf);
            fast_print!(
                &mut out, 1, 2, @sep '.', 3, 4; 5, 6, @sp @it 7..=10;
                @tup (1, 2, 3); @flush 4, @fmt ("{}?{}", 5, 6.7);
                { @ns @it 8..=10; @lf @it 11..=13 },
                @it2d (0..3).map(|i| (14..=15).map(move |j| i * 2 + j));
                @ns @ittup (0..2).map(|i| (i * 2 + 20, i * 2 + 21));
                @flush,
                @bw (b'a' [0, 1, 2].iter().cloned());
                @b b"xy";
                @sp @it1 (0..2)
            );
            out.flush();
        }
        let expected = r#"1 2.3.4
5.6 7 8 9 10
1 2 3
4 5?6.7
8910
11
12
13 14 15
16 17
18 19
2021
2223
abc
xy
1 2
"#;
        assert_eq!(expected, String::from_utf8_lossy(&buf));
    }
}
