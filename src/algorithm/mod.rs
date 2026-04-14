//! algorithm

use crate::algebra::{Field, Group, Invertible, Magma, Monoid, Unital};
use crate::data_structure::{BitSet, UnionFindBase, union_find};
use crate::graph::UndirectedSparseGraph;
use crate::math::{Convolve998244353, ConvolveSteps, Matrix};
use crate::num::{MInt, MIntBase, One, RangeBoundsExt, URational, Unsigned, Zero, montgomery};
use crate::tools::{RandomSpec, SerdeByteStr, Xorshift};
use crate::tree::LevelAncestor;
pub use self::automata_learning::*;
pub use self::baby_step_giant_step::baby_step_giant_step;
pub use self::binary_search::{Bisect, SliceBisectExt, binary_search, parallel_binary_search};
pub use self::bitdp::{BitDpExt, Combinations, Subsets};
pub use self::cartesian_tree::CartesianTree;
pub use self::chromatic_number::IndependentSubSet;
pub use self::combinations::SliceCombinationsExt;
pub use self::convex_hull_trick::ConvexHullTrick;
pub use self::doubling::{Doubling, FunctionalGraphDoubling};
pub use self::esper::{EsperEstimator, EsperSolver};
pub use self::horn_satisfiability::HornSatisfiability;
pub use self::impartial_game::{ImpartialGame, ImpartialGameAnalyzer, ImpartialGamer};
pub use self::number_of_increasing_sequences_between::{
    number_of_increasing_sequences_between, number_of_increasing_sequences_between_998244353,
};
pub use self::other::*;
pub use self::partisan_game::{PartisanGame, PartisanGameAnalyzer, PartisanGamer};
pub use self::quotient_index::{CeilQuotientIndex, FloorQuotientIndex};
pub use self::rho_path::RhoPath;
pub use self::solve_01_on_tree::solve_01_on_tree;
pub use self::sort::SliceSortExt;
pub use self::sqrt_decomposition::{
    RangeUpdateRangeFoldSqrtDecomposition, SqrtDecomposition, SqrtDecompositionBuckets,
};
pub use self::stern_brocot_tree::{SbtNode, SbtPath, SternBrocotTree, rational_binary_search};
pub use self::ternary_search::{golden_ternary_search, piecewise_ternary_search, ternary_search};
pub use self::xorbasis::XorBasis;
pub use self::zero_sum_game::{ZeroSumGame, ZeroSumGameAnalyzer, ZeroSumGamer};
mod automata_learning;
mod baby_step_giant_step;
mod binary_search;
mod bitdp;
mod cartesian_tree;
mod chromatic_number;
mod combinations;
mod convex_hull_trick;
mod doubling;
mod esper;
mod horn_satisfiability;
mod impartial_game;
mod mo_algorithm;
mod number_of_increasing_sequences_between;
mod other;
mod partisan_game;
mod quotient_index;
mod rho_path;
mod solve_01_on_tree;
mod sort;
mod sqrt_decomposition;
mod stern_brocot_tree;
mod syakutori;
pub mod ternary_search;
mod xorbasis;
mod zero_sum_game;
