use crate::tools::Xorshift;
pub use self::beam_search::{ModifiableState, beam_search};
pub use self::simulated_annealing::SimulatedAnnealing;
mod beam_search;
mod simulated_annealing;
