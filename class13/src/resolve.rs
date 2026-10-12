//! Deterministic symbol selection, before assigning output addresses.
use crate::symbols::{Definition, Symbol};
use std::collections::BTreeMap;

const STB_LOCAL: u8 = 0;
const STB_GLOBAL: u8 = 1;
const STB_WEAK: u8 = 2;

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
pub fn build_globals(files: &[Vec<Symbol<'_>>]) -> Result<Globals, Error> {
    let mut globals = Globals::new();
    for (file, table) in files.iter().enumerate() {
        for (symbol, definition) in table.iter().enumerate() {
            let id = SymbolId { file, symbol };
            validate_symbol(definition, id)?;
            if definition.binding == STB_LOCAL || definition.definition == Definition::Undefined {
                continue;
            }
            let incoming = if definition.definition == Definition::Common {
                Selection::Common {
                    first: id,
                    size: definition.size,
                    alignment: definition.value.max(1),
                }
            } else {
                Selection::Definition(id)
            };
            if let Some(existing) = globals.get_mut(definition.name) {
                merge_selection(files, existing, incoming, definition.name)?;
            } else {
                globals.insert(definition.name.to_vec(), incoming);
            }
        }
    }
    Ok(globals)
}

fn validate_symbol(symbol: &Symbol<'_>, id: SymbolId) -> Result<(), Error> {
    if !matches!(symbol.binding, STB_LOCAL | STB_GLOBAL | STB_WEAK) {
        return Err(Error::UnsupportedBinding(id));
    }
    if symbol.definition == Definition::Common {
        if symbol.binding != STB_GLOBAL {
            return Err(Error::UnsupportedCommonBinding(id));
        }
        if symbol.value != 0 && !symbol.value.is_power_of_two() {
            return Err(Error::InvalidCommonAlignment(id));
        }
    }
    if symbol.binding != STB_LOCAL && symbol.name.is_empty() {
        return Err(Error::EmptyGlobalName(id));
    }
    Ok(())
}

fn merge_selection(
    files: &[Vec<Symbol<'_>>],
    existing: &mut Selection,
    incoming: Selection,
    name: &[u8],
) -> Result<(), Error> {
    // Both identities originate from the current traversal of these same files.
    // A definition's binding is therefore validated and its indices are in range.
    let is_strong = |id: SymbolId| files[id.file][id.symbol].binding == STB_GLOBAL;
    match (&mut *existing, incoming) {
        (Selection::Definition(first), Selection::Definition(second)) => {
            if is_strong(*first) && is_strong(second) {
                return Err(Error::Duplicate {
                    name: name.to_vec(),
                    first: *first,
                    second,
                });
            }
            if is_strong(second) {
                *first = second;
            }
        }
        (
            Selection::Common {
                size, alignment, ..
            },
            Selection::Common {
                size: incoming_size,
                alignment: incoming_alignment,
                ..
            },
        ) => {
            *size = (*size).max(incoming_size);
            *alignment = (*alignment).max(incoming_alignment);
        }
        (Selection::Definition(id), incoming @ Selection::Common { .. }) => {
            if !is_strong(*id) {
                *existing = incoming;
            }
        }
        (Selection::Common { .. }, Selection::Definition(id)) => {
            if is_strong(id) {
                *existing = Selection::Definition(id);
            }
        }
    }
    Ok(())
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
    let s = files
        .get(id.file)
        .and_then(|t| t.get(id.symbol))
        .ok_or(Error::InvalidIndex(id))?;
    if s.binding == STB_LOCAL {
        if s.definition != crate::symbols::Definition::Undefined {
            return Ok(Binding::Selected(Selection::Definition(id)));
        }
    } else {
        if let Some(selected) = globals.get(s.name) {
            return Ok(Binding::Selected(selected.clone()));
        }
        if s.binding == STB_WEAK {
            return Ok(Binding::Zero);
        }
    }
    Err(Error::Undefined {
        name: s.name.to_vec(),
        reference: id,
    })
}
