pub use self::beam_search::{beam_search, ModifiableState};
pub use self::simulated_annealing::SimulatedAnnealing;
use crate::tools::Xorshift;
mod beam_search;
mod simulated_annealing;
