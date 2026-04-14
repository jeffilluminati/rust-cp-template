//! modint

use crate::{
    num::{BarrettReduction, One, Zero},
    tools::{IterScan, SerdeByteStr},
};
pub use mint_base::{MInt, MIntBase, MIntConvert};
mod mint_base;
pub mod mint_basic;
pub mod montgomery;

mod random_spec {
    use super::*;
    use crate::tools::{RandomSpec, Xorshift};
    use std::ops::{RangeFull, RangeTo};

    impl<M> RandomSpec<MInt<M>> for RangeFull
    where
        M: MIntBase,
        RangeTo<M::Inner>: RandomSpec<M::Inner>,
    {
        fn rand(&self, rng: &mut Xorshift) -> MInt<M> {
            MInt::<M>::new_unchecked(rng.random(..M::get_mod()))
        }
    }
}
