//! Deterministic symbol selection, before assigning output addresses.
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolId {
    pub file: usize,
    pub symbol: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Definition(SymbolId),
    Common {
        first: SymbolId,
        size: u64,
        alignment: u64,
    },
}
pub type Globals = BTreeMap<Vec<u8>, Selection>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    Selected(Selection),
    Zero,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    InvalidIndex(SymbolId),
    UnsupportedBinding(SymbolId),
    UnsupportedCommonBinding(SymbolId),
    InvalidCommonAlignment(SymbolId),
    EmptyGlobalName(SymbolId),
    Duplicate {
        name: Vec<u8>,
        first: SymbolId,
        second: SymbolId,
    },
    Undefined {
        name: Vec<u8>,
        reference: SymbolId,
    },
}

/// Scan files and their symbols in input order. Validate every symbol's binding
/// (LOCAL=0, GLOBAL=1, WEAK=2), COMMON binding (GLOBAL only), and COMMON alignment
/// (zero or power of two; normalize zero to one). Non-local names must be nonempty.
/// Local symbols never enter Globals. Undefined symbols supply no definition.
/// Selection priority: strong regular/absolute > global COMMON > weak definition.
/// Duplicate strong definitions fail even if unreferenced. Merge COMMON size and
/// alignment independently using maxima, preserving the first COMMON's identity.
/// Equal weak definitions keep the first input. Return an owned, byte-keyed map.
/// No address allocation, dynamic interposition, or visibility changes occur here.
pub fn build_globals(files: &[Vec<crate::symbols::Symbol<'_>>]) -> Result<Globals, Error> {
    let _ = files;
    todo!("C3.1: validate symbols and select global definitions deterministically")
}

/// Resolve one reference against a map built from the SAME unmodified files.
/// A defined LOCAL binds to its own identity, regardless of name collisions.
/// An undefined LOCAL fails. A non-local binds to the global selection by name,
/// including when its own input symbol is a weak definition replaced by strong.
/// An unresolved WEAK binds to Zero. Any other unresolved reference fails.
/// Index errors are returned, not panics. Call build_globals before this function.
/// Undefined symbols not actually referenced need not be passed to bind.
pub fn bind(
    files: &[Vec<crate::symbols::Symbol<'_>>],
    globals: &Globals,
    id: SymbolId,
) -> Result<Binding, Error> {
    let _ = (files, globals, id);
    todo!("C3.2: bind references using local identity or global selection")
}
