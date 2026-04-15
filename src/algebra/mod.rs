//! algebra

pub use self::action::*;
pub use self::lazy_map::*;
pub use self::magma::*;
pub use self::operations::*;
pub use self::ring::*;
pub use self::ring_operations::*;
use crate::num::{Bounded, One, Zero};
mod action;
mod lazy_map;
mod magma;
mod operations;
mod ring;
mod ring_operations;
