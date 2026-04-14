//! mathematical datas

use crate::algebra::{
    AddMulOperation, Associative, Field, Group, Invertible, Magma, Monoid, Ring, SemiRing, Unital,
};
use crate::array;
use crate::num::{
    BarrettReduction, Complex, ExtendedGcd, MInt, MIntBase, MIntConvert, One, RangeBoundsExt,
    Signed, Unsigned, Wrapping, Zero, montgomery,
};
use crate::tools::{AssociatedValue, PartialIgnoredOrd, SerdeByteStr, Xorshift};
#[cfg(target_arch = "x86_64")]
use crate::tools::{SimdBackend, simd_backend};
pub use self::arbitrary_mod_binomial::ArbitraryModBinomial;
pub use self::array_vec::{ArrayVec, ToArrayVec, ToArrayVecScalar};
pub use self::berlekamp_massey::berlekamp_massey;
pub use self::bitwise_transform::bitwise_transform;
pub use self::bitwiseand_convolve::{
    BitwiseandConvolve, OnlineSupersetMobiusTransform, OnlineSupersetZetaTransform,
};
pub use self::bitwiseor_convolve::{
    BitwiseorConvolve, OnlineSubsetMobiusTransform, OnlineSubsetZetaTransform,
};
pub use self::bitwisexor_convolve::BitwisexorConvolve;
pub use self::black_box_matrix::{
    BlackBoxMIntMatrix, BlackBoxMatrix, BlackBoxMatrixImpl, SparseMatrix,
};
pub use self::convolve_steps::ConvolveSteps;
pub use self::discrete_logarithm::{discrete_logarithm, discrete_logarithm_prime_mod};
pub use self::factorial::MemorizedFactorial;
pub use self::fast_fourier_transform::ConvolveRealFft;
pub use self::floor_sum::{
    floor_power_sum, floor_sum, floor_sum_i64, floor_sum_polynomial, floor_sum_polynomial_i64,
    floor_sum_range_freq,
};
pub use self::formal_power_series::{
    FormalPowerSeries, FormalPowerSeriesCoefficient, FormalPowerSeriesCoefficientSqrt, Fps,
    Fps998244353,
};
pub use self::garner::Garner;
pub use self::gcd::*;
pub use self::gcd_convolve::GcdConvolve;
pub use self::lagrange_interpolation::{lagrange_interpolation, lagrange_interpolation_polynomial};
pub use self::lcm_convolve::LcmConvolve;
pub use self::linear_congruence::{solve_linear_congruence, solve_simultaneous_linear_congruence};
pub use self::linear_diophantine::solve_linear_diophantine;
pub use self::matrix::Matrix;
pub use self::miller_rabin::{miller_rabin, miller_rabin_with_br};
pub use self::mint_matrix::MIntMatrix;
pub use self::number_theoretic_transform::{
    Convolve, Convolve998244353, MIntConvolve, NttReuse, U64Convolve,
};
pub use self::polynomial::*;
pub use self::pow_prec::PowPrec;
pub use self::prime::*;
pub use self::prime_factors::{divisors, prime_factors, prime_factors_flatten};
pub use self::prime_list::{PrimeList, with_prime_list};
pub use self::prime_table::PrimeTable;
pub use self::primitive_root::{check_primitive_root, primitive_root};
pub use self::quotient_array::QuotientArray;
pub use self::relaxed_convolution::RelaxedConvolution;
pub use self::subset_convolve::SubsetConvolve;
mod arbitrary_mod_binomial;
mod array_vec;
mod berlekamp_massey;
mod bitwise_transform;
mod bitwiseand_convolve;
mod bitwiseor_convolve;
mod bitwisexor_convolve;
mod black_box_matrix;
mod convolve_steps;
mod discrete_logarithm;
mod factorial;
mod fast_fourier_transform;
mod floor_sum;
mod formal_power_series;
mod garner;
mod gcd;
mod gcd_convolve;
mod lagrange_interpolation;
mod lcm_convolve;
mod linear_congruence;
mod linear_diophantine;
mod matrix;
mod miller_rabin;
mod mint_matrix;
mod mod_sqrt;
mod number_theoretic_transform;
mod polynomial;
mod pow_prec;
mod prime;
mod prime_factors;
mod prime_list;
mod prime_table;
mod primitive_root;
mod quotient_array;
mod relaxed_convolution;
mod subset_convolve;
#[allow(dead_code)]
#[doc(hidden)]
enum ZetaTransformSnippets {}
