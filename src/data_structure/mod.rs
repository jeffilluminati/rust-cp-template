//! data structures

pub use self::accumulate::{Accumulate, Accumulate2d, AccumulateKd};
pub use self::allocator::{Allocator, BoxAllocator, MemoryPool};
pub use self::binary_indexed_tree::BinaryIndexedTree;
pub use self::binary_indexed_tree_2d::BinaryIndexedTree2D;
pub use self::bit_vector::{BitVector, RankSelectDictionaries};
pub use self::bitset::BitSet;
pub use self::compress::{Compressor, HashCompress, VecCompress};
pub use self::compressed_binary_indexed_tree::{
    CompressedBinaryIndexedTree, CompressedBinaryIndexedTree1d, CompressedBinaryIndexedTree2d,
    CompressedBinaryIndexedTree3d, CompressedBinaryIndexedTree4d,
};
pub use self::compressed_segment_tree::{
    CompressedSegmentTree, CompressedSegmentTree1d, CompressedSegmentTree2d,
    CompressedSegmentTree3d, CompressedSegmentTree4d,
};
pub use self::container::{
    BTreeMapFactory, Container, ContainerEntry, ContainerFactory, HashMapFactory,
    HashMapFactoryWithCapacity,
};
pub use self::counter::{BTreeCounter, HashCounter};
pub use self::disjoint_sparse_table::DisjointSparseTable;
pub use self::fibonacci_hash::{
    FibHashMap, FibHashSet, FibonacciHasher, FibonacciHasheru32, FibonacciHasheru64,
};
pub use self::kdtree::Static2DTree;
pub use self::lazy_segment_tree::LazySegmentTree;
pub use self::lazy_segment_tree_map::LazySegmentTreeMap;
pub use self::line_set::LineSet;
pub use self::pairing_heap::PairingHeap;
pub use self::partially_retroactive_priority_queue::PartiallyRetroactivePriorityQueue;
pub use self::persistent_segment_tree::PersistentSegmentTree;
pub use self::range_ap_add::RangeArithmeticProgressionAdd;
pub use self::range_frequency::RangeFrequency;
pub use self::range_map::{RangeMap, RangeSet};
pub use self::range_minimum_query::RangeMinimumQuery;
pub use self::segment_tree::SegmentTree;
pub use self::segment_tree_map::SegmentTreeMap;
pub use self::sliding_window_aggregation::{DequeAggregation, QueueAggregation};
pub use self::slope_trick::SlopeTrick;
pub use self::sparse_set::SparseSet;
pub use self::splay_tree::{SplayMap, SplaySequence};
pub use self::submask_range_query::SubmaskRangeQuery;
pub use self::transducer::*;
pub use self::treap::{Treap, TreapData};
pub use self::trie::Trie;
pub use self::union_find::{
    MergingUnionFind, PotentializedUnionFind, UndoableUnionFind, UnionFind, UnionFindBase,
};
pub use self::vec_map::{FixedVecMapFactory, VecMap, VecMapFactory, VecMapFactoryWithCapacity};
pub use self::wavelet_matrix::WaveletMatrix;
use crate::algebra::{
    AbelianGroup, AbelianMonoid, AdditiveOperation, Associative, EmptyAct, Group, LazyMapMonoid,
    Magma, MaxOperation, MinOperation, Monoid, MonoidAct, SemiGroup, Unital,
};
use crate::algorithm::{BitDpExt, SliceBisectExt};
use crate::num::{Bounded, RangeBoundsExt};
use crate::tools::{Comparator, Xorshift, comparator};
mod accumulate;
mod allocator;
mod binary_indexed_tree;
mod binary_indexed_tree_2d;
pub mod binary_search_tree;
mod bit_vector;
mod bitset;
mod compress;
mod compressed_binary_indexed_tree;
mod compressed_segment_tree;
mod container;
mod counter;
mod disjoint_sparse_table;
mod fibonacci_hash;
mod kdtree;
mod lazy_segment_tree;
mod lazy_segment_tree_map;
mod line_set;
mod pairing_heap;
pub mod partially_retroactive_priority_queue;
mod persistent_segment_tree;
mod range_ap_add;
mod range_frequency;
mod range_map;
mod range_minimum_query;
mod segment_tree;
mod segment_tree_map;
mod sliding_window_aggregation;
mod slope_trick;
mod sparse_set;
pub mod splay_tree;
pub mod submask_range_query;
mod transducer;
pub mod treap;
mod trie;
pub mod union_find;
mod vec_map;
mod wavelet_matrix;
