//! string algorithms

pub use self::knuth_morris_pratt::KnuthMorrisPratt;
pub use self::rolling_hash::{
    Gf2_63x1, Gf2_63x2, Gf2_63x3, HashedRangeChained, Mersenne61x1, Mersenne61x2, Mersenne61x3,
    RollingHasher,
};
pub use self::string_search::{MultipleStringSearch, StringSearch};
pub use self::suffix_array::SuffixArray;
pub use self::suffix_automaton::SuffixAutomaton;
pub use self::suffix_tree::{MultipleSuffixTree, SuffixTree};
pub use self::wildcard_pattern_matching::wildcard_pattern_matching;
pub use self::z_algorithm::Zarray;
use crate::algebra::{Gf2_63, Invertible, Mersenne61, Monoid, Ring, SemiRing};
use crate::data_structure::RangeMinimumQuery;
use crate::math::{Convolve, ConvolveSteps};
use crate::num::{montgomery, Zero};
use crate::tools::Xorshift;
mod knuth_morris_pratt;
pub mod rolling_hash;
mod string_search;
mod suffix_array;
mod suffix_automaton;
mod suffix_tree;
mod wildcard_pattern_matching;
mod z_algorithm;
