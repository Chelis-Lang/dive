//! The configuration's `_PERIODIC` entry.
//!
//! TLC binds the named definition (sampled after substitutions and before
//! constant definitions are cached, as `SpecProcessor.processPeriodic` does)
//! and evaluates it on its wall-clock progress timer. dive binds it with
//! TLC's errors, then refuses to run it: the hook is a named unsupported
//! feature until it is evaluated at deterministic level boundaries (D-014).

use dive_ir::DefId;

use crate::{BindError, Ec, bind::Binder, tables::Look};

/// Resolves the configured `_PERIODIC` name to its zero-arity definition.
///
/// # Errors
/// TLC's `TlcConfigSpecifiedNotDefined` for an unknown name and
/// `TlcConfigIdRequiresNoArg` for an operator with parameters. An entry that
/// is not a definition is refused by name: TLC fails there with a Java
/// `ClassCastException`, which has no message code to reproduce.
pub(crate) fn resolve(b: &Binder<'_>, name: &str, entry: Option<&Look>) -> crate::bind::R<DefId> {
    let definition = match entry {
        Some(Look::Def(definition)) => *definition,
        None => {
            return Err(
                BindError::tlc(Ec::TlcConfigSpecifiedNotDefined, &["periodic", name]).into(),
            );
        }
        Some(_) => {
            return Err(BindError::unsupported(
                "non-definition _PERIODIC entry (TLC ClassCastException boundary)",
            )
            .into());
        }
    };
    if !b.t.ir.def(definition).params.is_empty() {
        return Err(BindError::tlc(Ec::TlcConfigIdRequiresNoArg, &["periodic", name]).into());
    }
    Ok(definition)
}
