//! Configuration-owned pre-constant periodic definition snapshot.
use std::sync::Arc;

use dive_config::ModelConfig;
use dive_ir::{DefId, Ir};

use crate::{BindError, BindOptions, Ec, Failure, Model, Pred, bind::Binder, tables::Look};

pub(crate) struct Role {
    name: String,
    definition: DefId,
    predicate: Pred,
}

/// A periodic body selected by actual configuration binding, together with the
/// exact model and evaluator ownership used to interpret it. Fields are private:
/// a caller-supplied predicate cannot replace this selection.
///
/// This is diagnostic binding, not ordinary scheduling or a model proof. Like
/// [`crate::bind`], constructing it may execute constant startup definitions.
/// Callers requiring callback-free admission must guard source effects before
/// binding; the bounded liveness snapshot wrapper does that.
pub struct PeriodicSnapshot {
    pub(crate) model: Model,
    pub(crate) role: Role,
}
impl PeriodicSnapshot {
    /// Exact producer-added refusal retained on the exposed model. Ordinary
    /// checking cannot implement the configuration hook through this snapshot.
    pub const ORDINARY_REFUSAL: &'static str =
        "_PERIODIC diagnostic snapshot: ordinary scheduling is unavailable";
    /// Exact bound model belonging to this snapshot.
    #[must_use]
    pub fn model(&self) -> &Model {
        &self.model
    }
    /// Original configured role spelling, distinct from the selected definition.
    #[must_use]
    pub fn configured_name(&self) -> &str {
        &self.role.name
    }
    /// Pre-constant selected definition, including configuration substitution.
    #[must_use]
    pub fn definition(&self) -> DefId {
        self.role.definition
    }
    /// Original selected definition body and its owned evaluation context.
    #[must_use]
    pub fn predicate(&self) -> &Pred {
        &self.role.predicate
    }
}

/// Bind a diagnostic configuration-owned `_PERIODIC` snapshot. Ordinary
/// [`crate::bind`] continues to refuse this role. The definition table is sampled
/// after substitutions and before constant caching, as in `processPeriodic`.
///
/// This shares all underlying binding stages with ordinary binding. It does not
/// admit host effects safely on its own: startup evaluation is unchanged. A
/// non-definition snapshot entry is refused by name; TLC's corresponding Java
/// `ClassCastException` is not invented as a TLC message code.
///
/// # Errors
/// Original binding error/output, missing or nonzero-arity periodic name, an
/// unsupported non-definition entry, or absence of the requested diagnostic role.
pub fn bind_periodic_snapshot(
    ir: Arc<Ir>,
    cfg: &ModelConfig,
    opts: &BindOptions,
) -> Result<PeriodicSnapshot, Box<Failure>> {
    let (mut model, role) = crate::bind_in(ir, cfg, opts, None, true)?;
    let role = role.ok_or_else(|| {
        Box::new(Failure {
            error: BindError::unsupported("missing diagnostic periodic snapshot"),
            output: Vec::new(),
        })
    })?;
    model.unsupported.push(crate::Unsupported {
        kind: crate::UnsupportedKind::Feature,
        feature: PeriodicSnapshot::ORDINARY_REFUSAL.to_owned(),
    });
    Ok(PeriodicSnapshot { model, role })
}

pub(crate) fn resolve(b: &Binder<'_>, entry: Option<&Look>) -> Result<Role, BindError> {
    let name = b
        .cfg
        .periodic
        .as_ref()
        .ok_or_else(|| BindError::unsupported("diagnostic _PERIODIC role is absent"))?
        .text
        .clone();
    let definition = match entry {
        Some(Look::Def(definition)) => *definition,
        None => {
            return Err(BindError::tlc(
                Ec::TlcConfigSpecifiedNotDefined,
                &["periodic", &name],
            ));
        }
        Some(_) => {
            return Err(BindError::unsupported(
                "non-definition periodic snapshot (TLC ClassCastException boundary)",
            ));
        }
    };
    let def = b.t.ir.def(definition);
    if !def.params.is_empty() {
        return Err(BindError::tlc(
            Ec::TlcConfigIdRequiresNoArg,
            &["periodic", &name],
        ));
    }
    let scope = b
        .ev()
        .scope(definition)
        .map_err(|e| BindError::unsupported(e.to_string()))?;
    Ok(Role {
        name,
        definition,
        predicate: Pred::new(def.body, scope),
    })
}
