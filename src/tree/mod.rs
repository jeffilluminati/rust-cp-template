//! tree algorithms

use crate::{
    algebra::{Magma, Monoid, Unital},
    data_structure::RangeMinimumQuery,
    graph::UndirectedSparseGraph,
    math::{ConvolveSteps, U64Convolve},
    tools::{RandomSpec, Xorshift},
};
pub use self::centroid_decomposition::ContourQueryRange;
pub use self::generator::*;
pub use self::heavy_light_decomposition::HeavyLightDecomposition;
pub use self::level_ancestor::LevelAncestor;
pub use self::rerooting::ReRooting;
pub use self::static_top_tree::{Cluster, MonoidCluster, StaticTopTree, StaticTopTreeDp};
pub use self::tree_center::*;
pub use self::tree_hash::TreeHasher;
mod centroid_decomposition;
mod depth;
mod euler_tour;
mod generator;
mod heavy_light_decomposition;
mod level_ancestor;
mod rerooting;
mod static_top_tree;
mod tree_center;
mod tree_centroid;
mod tree_dp;
mod tree_hash;
mod tree_order;
