//! Configuration-owned pre-constant periodic definition snapshot.
use std::sync::Arc;

use dive_config::ModelConfig;
use dive_ir::{DefId, Ir};

use crate::{BindError, BindOptions, Ec, Failure, Model, Pred, bind::Binder, tables::Look};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Ordinary,
    Required,
    Optional,
}

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
    let (mut model, role) = crate::bind_in(ir, cfg, opts, None, Mode::Required)?;
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

/// Configuration-owned optional periodic selection. Absence comes only from the
/// original configuration, never a supplied predicate or an inferred TRUE body.
/// The exposed model retains the ordinary-scheduling refusal even when absent.
pub struct OptionalPeriodicSnapshot {
    model: Model,
    role: Option<Role>,
}
impl OptionalPeriodicSnapshot {
    /// Exact model and evaluator owned by this selection.
    #[must_use]
    pub fn model(&self) -> &Model {
        &self.model
    }
    /// Original configured role, or genuine configuration absence.
    #[must_use]
    pub fn configured_name(&self) -> Option<&str> {
        self.role.as_ref().map(|r| r.name.as_str())
    }
    /// Actual selected pre-constant definition, without a synthetic fallback.
    #[must_use]
    pub fn definition(&self) -> Option<DefId> {
        self.role.as_ref().map(|r| r.definition)
    }
    /// Original owned selected body, if a role was configured.
    #[must_use]
    pub fn predicate(&self) -> Option<&Pred> {
        self.role.as_ref().map(|r| &r.predicate)
    }
}
/// Bind an optional diagnostic selection through the same actual binder stages
/// and pre-constant definition snapshot as the mandatory API. Constant startup
/// evaluation is unchanged; callers needing pre-effect admission must use the
/// bounded liveness wrapper. This never enables ordinary checking.
///
/// # Errors
/// Original binding errors, or invalid/non-definition/nonzero-arity configured
/// roles. An absent role succeeds as genuine absence.
pub fn bind_optional_periodic_snapshot(
    ir: Arc<Ir>,
    cfg: &ModelConfig,
    opts: &BindOptions,
) -> Result<OptionalPeriodicSnapshot, Box<Failure>> {
    let (mut model, role) = crate::bind_in(ir, cfg, opts, None, Mode::Optional)?;
    model.unsupported.push(crate::Unsupported {
        kind: crate::UnsupportedKind::Feature,
        feature: PeriodicSnapshot::ORDINARY_REFUSAL.to_owned(),
    });
    Ok(OptionalPeriodicSnapshot { model, role })
}
