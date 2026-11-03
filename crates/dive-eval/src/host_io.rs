//! The admitted `IOUtils` input kernels; all operating-system work stays in hosts.

use std::sync::Arc;

use dive_ir::{Arg, ExprId};
use dive_value::{Value, ValueOrder};

use crate::core::Core;
use crate::env::{Env, St};
use crate::inputs::{InputRequest, InputResponse};
use crate::{ErrorClass, EvalError, Result};

impl Core {
    pub(crate) fn remember_input_refusal(&self, error: &EvalError) {
        if error.class() == ErrorClass::Unsupported {
            self.input_refusal
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get_or_insert_with(|| error.message().to_owned());
        }
    }

    pub(crate) fn observed_input(&self, result: Result<Value>) -> Result<Value> {
        if let Err(error) = &result {
            self.remember_input_refusal(error);
        }
        result
    }

    fn input(&self, request: &InputRequest) -> Result<InputResponse> {
        self.host
            .get()
            .and_then(|host| host.input(request))
            .ok_or_else(|| EvalError::unsupported("IOUtils external input context is unavailable"))?
            .map_err(crate::host::host_error)
    }

    pub(crate) fn io_environment(&self) -> Result<Value> {
        if self.ctx.order == ValueOrder::Tlc
            && self
                .text_input_used
                .load(std::sync::atomic::Ordering::Relaxed)
            && !self
                .environment_initialized
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err(EvalError::unsupported(
                "IOEnv TLC construction order after a TXT request is not implemented; capture the environment first",
            ));
        }
        let InputResponse::Environment(entries) = self.input(&InputRequest::Environment)? else {
            return Err(EvalError::unsupported("IOEnv external response kind"));
        };
        if entries.is_empty() {
            return Err(EvalError::unsupported(
                "IOEnv empty RecordValue representation is not implemented",
            ));
        }
        if entries.len() > crate::inputs::MAX_ENV_ENTRIES
            || entries.iter().fold(0usize, |n, (key, value)| {
                n.saturating_add(key.len()).saturating_add(value.len())
            }) > crate::inputs::MAX_INPUT_BYTES
        {
            return Err(EvalError::unsupported("IOEnv input bounds"));
        }
        if self.ctx.order == ValueOrder::Tlc {
            let mut unknown = std::collections::BTreeSet::new();
            for (key, value) in &entries {
                for text in [key.as_str(), value.as_str()] {
                    if self.names.get(text).is_none() {
                        unknown.insert(text);
                        if unknown.len() > 1 {
                            return Err(EvalError::unsupported(
                                "IOEnv TLC construction order for multiple new environment strings is not implemented",
                            ));
                        }
                    }
                }
            }
        }
        let fields = entries
            .into_iter()
            .map(|(name, text)| (self.names.intern(&name), Value::string(&self.names, &text)))
            .collect();
        let value = Value::record(fields, self.ctx).map_err(EvalError::from)?;
        self.environment_initialized
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(value)
    }

    pub(crate) fn io_text_deserialize(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        args: &[Arg],
        _origin: ExprId,
    ) -> Result<Value> {
        let [
            Arg::Expr { expr: path, .. },
            Arg::Expr { expr: options, .. },
        ] = args
        else {
            return Err(EvalError::unsupported(
                "IOUtils!Deserialize needs two expression arguments",
            ));
        };
        // Pinned textDeserialize evaluates options before the file path. Only
        // its successful, well-typed TXT/UTF-8 path is implemented here. The
        // Java exception-to-record cases remain named refusals, never guessed.
        let evaluated = self.eval_v(env, st, *options);
        let options = input_argument(evaluated)?;
        let Value::Fcn(options) = options else {
            return Err(EvalError::unsupported(
                "IOUtils!Deserialize non-record options",
            ));
        };
        if !options.is_record_literal() {
            return Err(EvalError::unsupported(
                "IOUtils!Deserialize non-record options",
            ));
        }
        let format = options.apply(&Value::string(&self.names, "format"));
        let charset = options.apply(&Value::string(&self.names, "charset"));
        if !matches!(format, Some(Value::Str(value)) if value.as_str() == "TXT")
            || !matches!(charset, Some(Value::Str(value)) if value.as_str() == "UTF-8")
        {
            return Err(EvalError::unsupported(
                "IOUtils!Deserialize supports TXT with charset UTF-8",
            ));
        }
        let path = input_argument(self.eval_v(env, st, *path))?;
        let Value::Str(path) = path else {
            return Err(EvalError::unsupported(
                "IOUtils!Deserialize non-string path",
            ));
        };
        if path.as_str().contains('\0') {
            return Err(EvalError::unsupported(
                "IOUtils!Deserialize invalid-path error records are not implemented",
            ));
        }
        let InputResponse::Text(text) =
            self.input(&InputRequest::TextFile(path.as_str().to_owned()))?
        else {
            return Err(EvalError::unsupported("Deserialize external response kind"));
        };
        if text.len() > crate::inputs::MAX_INPUT_BYTES {
            return Err(EvalError::unsupported("Deserialize text input byte limit"));
        }
        Value::record(
            vec![
                (self.names.intern("exitValue"), Value::Int(0)),
                (
                    self.names.intern("stdout"),
                    Value::string(&self.names, &text),
                ),
                (self.names.intern("stderr"), Value::string(&self.names, "")),
            ],
            self.ctx,
        )
        .map_err(Into::into)
    }
}

fn input_argument(value: Result<Value>) -> Result<Value> {
    value.map_err(|error| match error.class() {
        ErrorClass::Interrupted | ErrorClass::ResourceLimit | ErrorClass::Bug => error,
        _ => EvalError::unsupported(format!(
            "IOUtils!Deserialize argument-error records are not implemented ({error})"
        )),
    })
}
