//! Public independently initialized pure Core worker preparation.

use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::{ApiError, CheckRequest, DistributedUsage};

fn rejected(message: impl Into<String>) -> ApiError {
    ApiError::Rejected {
        message: message.into(),
    }
}

// The executable hash pins this representation. Include the full evaluator
// policy here without changing the existing ADR0141 checkpoint identity or its
// compatibility rules; search admission/transport limits remain coordinator
// policy and never alter the worker's source semantics.
pub(crate) fn identity(
    ir: &dive_ir::Ir,
    model: &dive_config::ModelConfig,
    options: &dive_runtime::CheckOptions,
    config_hash: &str,
    opts: dive_core::Opts,
) -> Result<Vec<u8>, dive_runtime::Verdict> {
    let request = dive_runtime::checkpoint::Request {
        path: std::path::PathBuf::new(),
        action: dive_runtime::checkpoint::Action::PauseAfterLevels(0),
        counters: dive_runtime::checkpoint::Counters::Retain,
        config_source_sha256: Some(config_hash.into()),
    };
    let mut identity =
        dive_runtime::checkpoint::owned_identity(ir, model, options, &request, &opts.describe())?;
    identity.extend_from_slice(
        format!(
            "\ndistributed-policy-0151-v1/{:?}/{:?}",
            options.eval, options.symmetry_liveness
        )
        .as_bytes(),
    );
    Ok(identity)
}

/// An independently loaded, bound and compiled worker with no received-code or
/// host-I/O grants. Assigned source expansion requires an authenticated session;
/// local constant/ASSUME binding and compilation happen during preparation.
pub struct CoreWorker {
    inner: dive_explore::distributed::Worker,
}
impl CoreWorker {
    /// Serves authenticated work from the supplied listener until the caller
    /// stops it or `max_units` complete. This never creates a model verdict.
    ///
    /// # Errors
    /// Listener/setup failures. Invalid sessions are rejected in worker facts.
    pub fn serve(
        &self,
        listener: &TcpListener,
        max_units: Option<u64>,
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Result<DistributedUsage, ApiError> {
        std::thread::scope(|scope| {
            std::thread::Builder::new()
                .stack_size(dive_eval::RECOMMENDED_STACK)
                .spawn_scoped(scope, || self.inner.serve(listener, max_units, stop))
                .map_err(|e| rejected(format!("worker execution thread: {e}")))?
                .join()
                .map_err(|_| rejected("worker execution thread panicked"))?
                .map_err(|e| rejected(e.message()))
        })
    }
}

/// Independently prepares the same pure Core model as a distributed coordinator.
///
/// The source/config, executable, seed/fingerprint, value policy, Core switches
/// and compiled startup are pinned before receiving any work. The local key file
/// must hold exactly 32 secret bytes; work messages cannot choose paths or code.
/// Both worker and coordinator require explicit seed and fingerprint options.
///
/// ```no_run
/// use dive_api::{CheckRequest, prepare_core_worker};
/// use std::{net::TcpListener, path::Path, time::Duration};
/// let request = CheckRequest {
///     file: "Counter.tla".into(), seed: Some(0), fp: Some(0),
///     ..CheckRequest::default()
/// };
/// let worker = prepare_core_worker(&request, Path::new("worker.key"), Duration::from_secs(5))?;
/// let listener = TcpListener::bind("127.0.0.1:7801")?;
/// let facts = worker.serve(&listener, Some(5), &|| false)?;
/// assert!(facts.completed_units <= 5); // Work facts never prove the model.
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// Source/configuration errors, effects, unimplemented combinations, compilation,
/// private protocol admission, key-file or identity failures.
pub fn prepare_core_worker(
    request: &CheckRequest,
    key_file: &Path,
    timeout: Duration,
) -> Result<CoreWorker, ApiError> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, || prepare_inner(request, key_file, timeout))
            .map_err(|e| rejected(format!("worker preparation thread: {e}")))?
            .join()
            .map_err(|_| rejected("worker preparation thread panicked"))?
    })
}

fn admit_request(request: &CheckRequest) -> Result<(), ApiError> {
    if request.debugger.is_some() {
        return Err(ApiError::Unsupported {
            feature: "worker preparation with compiled debugger is not implemented".into(),
        });
    }
    if request.store_compact_bytes.is_some() {
        return Err(ApiError::Unsupported {
            feature: "worker preparation with Store reclamation is not implemented".into(),
        });
    }
    if request.tlc_coverage {
        return Err(ApiError::Unsupported {
            feature: "worker preparation with TLC logical coverage is not implemented".into(),
        });
    }
    if request.seed.is_none()
        || request.fp.is_none()
        || !request.host_access.is_empty()
        || request.distributed.is_some()
        || request.source_coverage
        || request.core_sample.is_some()
        || request.temporal != crate::TemporalMode::Ordinary
        || request.seen_disk.is_some()
        || request.candidate_memory_bytes.is_some()
        || request.candidate_directory.is_some()
        || request.frontier_memory_bytes.is_some()
        || request.frontier_directory.is_some()
        || request.predecessor_memory_bytes.is_some()
        || request.predecessor_directory.is_some()
        || request.sequential
        || request.chaos.is_some()
        || request.max_states.is_some()
        || request.max_depth.is_some()
        || request.time_limit != crate::BudgetSetting::Default
        || request.max_memory != crate::BudgetSetting::Default
        || request.workers.is_some_and(|workers| workers != 1)
    {
        return Err(ApiError::Unsupported{feature:"worker preparation requires explicit seed/fp and pure ordinary Core roles; coordinator search/storage/coverage options are unsupported".into()});
    }
    Ok(())
}

fn preparation_refusal(verdict: dive_runtime::Verdict) -> ApiError {
    ApiError::Unsupported {
        feature: match verdict {
            dive_runtime::Verdict::Unsupported { feature }
                if feature.starts_with("temporal properties") =>
            {
                format!(
                    "worker preparation with temporal properties is not implemented ({feature})"
                )
            }
            dive_runtime::Verdict::Unsupported { feature } => {
                format!("worker preparation: {feature}")
            }
            verdict => format!("worker preparation: {verdict:?}"),
        },
    }
}

fn prepare_inner(
    request: &CheckRequest,
    key_file: &Path,
    timeout: Duration,
) -> Result<CoreWorker, ApiError> {
    admit_request(request)?;
    let analysis = crate::analyze_file(
        &request.file,
        &crate::AnalyzeOptions {
            library_paths: request.library_paths.clone(),
            after_standard: request.module_paths.clone(),
        },
    )?;
    if !analysis.accepted() {
        return Err(rejected(analysis.render_human()));
    }
    let ir = Arc::new(
        dive_ir::lower(analysis.analysis())
            .map_err(|e| rejected(format!("worker lowering: {e:?}")))?,
    );
    let config_path = request
        .config
        .clone()
        .unwrap_or_else(|| request.file.with_extension("cfg"));
    let config = crate::parse_config_file(&config_path)?;
    let model = config
        .config()
        .ok_or_else(|| rejected(config.render_human()))?;
    let seed = request.seed.ok_or_else(|| rejected("worker seed"))?;
    let fp = request.fp.ok_or_else(|| rejected("worker fingerprint"))?;
    let mut options = if request.compat_tlc {
        dive_runtime::CheckOptions::tlc(fp, seed)
    } else {
        let mut options = dive_runtime::CheckOptions::dive(seed);
        options.fp_index = fp;
        options
    };
    options.no_deadlock = request.no_deadlock;
    options.store = match request.store {
        crate::Store::Fingerprint => dive_store::StoreMode::Fingerprint,
        crate::Store::Exact => dive_store::StoreMode::Exact,
    };
    if let Some(composition) = request.action_composition {
        options.eval.action_composition = composition;
    }
    if let Some(policy) = request.symmetry_liveness {
        options.symmetry_liveness = policy;
    }
    let opts = std::env::var("DIVE_CORE_OPT")
        .ok()
        .map_or_else(
            || Ok(dive_core::Opts::default()),
            |s| dive_core::Opts::parse(&s),
        )
        .map_err(|e| ApiError::Unsupported {
            feature: format!("worker Core switches: {e}"),
        })?;
    if let Some(feature) = crate::core_engine::option_refusal(&options, request.compat_tlc) {
        return Err(ApiError::Unsupported { feature });
    }
    let prepared =
        crate::core_engine::prepare(&ir, model, &options, request.compat_tlc, opts, false)
            .map_err(|r| preparation_refusal(r.verdict))?;

    if let Some(feature) =
        crate::core_engine::distributed_model_refusal(&prepared.plan, &prepared.model)
    {
        return Err(ApiError::Unsupported {
            feature: feature.replacen("distributed safety", "worker preparation", 1),
        });
    }
    let identity = identity(
        &ir,
        model,
        &options,
        &dive_runtime::checkpoint::config_source_hash(config.source_bytes()),
        opts,
    )
    .map_err(|v| ApiError::Unsupported {
        feature: match v {
            dive_runtime::Verdict::Unsupported { feature } => {
                format!("worker identity admission: {feature}")
            }
            verdict => format!("worker identity admission: {verdict:?}"),
        },
    })?;
    let inner = dive_explore::distributed::Worker::new(
        prepared.plan,
        prepared.store,
        opts,
        &identity,
        key_file,
        timeout,
    )
    .map_err(|e| rejected(e.message()))?;
    Ok(CoreWorker { inner })
}
