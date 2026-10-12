//! Capstone: robustness and differential validation of the whole linker.
//!
//! A linker reads untrusted files. Malformed or unsupported inputs should
//! produce diagnostics, never panics, out-of-bounds accesses, or hangs. This crate
//! provides a deterministic mutator and a survey harness; the native tests
//! point them at selected unwind/tls/icf/driver backends, and compare outputs
//! with GNU ld. Passing them usually means fixing earlier classes. README is
//! the contract.

/// The xorshift64* generator, so that every failure can be replayed exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Start from `seed`; seed 0 is replaced by `0x9e37_79b9_7f4a_7c15`
    /// because xorshift has no successor for an all-zero state.
    pub fn new(seed: u64) -> Self {
        let _ = seed;
        todo!("C15.1: seed the generator")
    }
    /// One step: `x ^= x >> 12; x ^= x << 25; x ^= x >> 27`, then return
    /// `x * 0x2545_f491_4f6c_dd1d` (wrapping); the state is the pre-multiply `x`.
    pub fn next_u64(&mut self) -> u64 {
        todo!("C15.1: advance xorshift64*")
    }
    /// `next_u64() % n` for `n > 0`. Slight modulo bias is acceptable here.
    pub fn below(&mut self, n: u64) -> u64 {
        let _ = n;
        todo!("C15.1: draw a bounded value")
    }
}

/// Apply one mutation chosen by `rng` to a copy of `input`. README fixes the
/// mutation menu and the exact order of random draws.
pub fn mutate(input: &[u8], rng: &mut Rng) -> Vec<u8> {
    let _ = (input, &rng);
    todo!("C15.1: derive a malformed variant of a valid input")
}

/// One mutated link that panicked: replay it with the same seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panic {
    pub iteration: u64,
    /// Index of the sample in the corpus.
    pub sample: usize,
    /// Index of the mutated input within that sample.
    pub input: usize,
    pub message: String,
}

/// Outcome counts of a survey.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub accepted: u64,
    pub rejected: u64,
    pub panics: Vec<Panic>,
}

/// Run `iterations` mutated links over `corpus` (each sample is a list of
/// inputs; none may be empty). `link` returns whether it accepted the
/// inputs. Panics are caught and recorded, not propagated, and the panic
/// hook is silenced for the duration. README defines the iteration schedule.
pub fn survey(
    corpus: &[Vec<Vec<u8>>],
    iterations: u64,
    seed: u64,
    link: &dyn Fn(&[&[u8]]) -> bool,
) -> Report {
    let _ = (corpus, iterations, seed, link);
    todo!("C15.2: survey mutated inputs and record every panic")
}
