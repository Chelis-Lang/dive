//! Dense ids into the [`crate::Ir`] arena.
//!
//! Ids are assigned by a deterministic traversal, so lowering the same input
//! always gives the same ids. They are not stable across edits; structural
//! hashes ([`crate::Def::hash`]) are.

use serde::{Deserialize, Serialize};

macro_rules! id {
    ($($(#[$doc:meta])* $name:ident;)*) => {
        $(
            $(#[$doc])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
            pub struct $name(pub u32);

            impl $name {
                /// The index this id stands for.
                #[must_use]
                pub fn index(self) -> usize {
                    self.0 as usize
                }

                pub(crate) fn from_index(i: usize) -> Self {
                    Self(u32::try_from(i).unwrap_or(u32::MAX))
                }
            }
        )*
    };
}

id! {
    /// A definition: operator, function, theorem or assumption, including
    /// instantiated (`I!Op`), `LET`-local and `LAMBDA` definitions.
    DefId;
    /// An expression node.
    ExprId;
    /// A state variable of the root module, in the root context's order.
    VarId;
    /// A `CONSTANT` declaration of any module.
    ConstId;
    /// A `CONSTANT` or `VARIABLE` declaration of any module.
    DeclId;
    /// A position in a definition's frame.
    Slot;
    /// A module, file-level or inner.
    ModuleId;
    /// A source file.
    FileId;
}
