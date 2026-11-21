"""Resource limits and result handling for native children of finite checking workers."""

import errno
import resource
import subprocess


class Stopped(RuntimeError):
    """A subprocess stopped before it produced a semantic result."""


def run_helper(command, label, memory_limited, **options):
    try:
        result = subprocess.run(command, text=True, capture_output=True, **options)
    except OSError as error:
        if error.errno in (errno.ENOMEM, errno.EAGAIN):
            raise Stopped(label + " exhausted process resources") from error
        raise
    if result.returncode:
        # A failed loader, allocator or thread startup can terminate before
        # Rust main returns a diagnostic. Never classify that as a model refusal.
        if result.returncode < 0:
            raise Stopped(f"{label} terminated by signal {-result.returncode}")
        if "cannot start a thread: " in result.stderr and any(
            f"(os error {code})" in result.stderr for code in (errno.ENOMEM, errno.EAGAIN)
        ):
            raise Stopped(label + " could not allocate a binder thread")
        if not result.stderr.startswith("Error: ") and not result.stdout.strip():
            reason = result.stderr.strip()[-500:] or f"exit {result.returncode}"
            if memory_limited:
                raise Stopped(f"{label} stopped under the memory budget: {reason}")
            raise RuntimeError(f"{label} failed without a result: {reason}")
    return result


def limit_memory(max_memory):
    if max_memory:
        _, hard = resource.getrlimit(resource.RLIMIT_AS)
        limit = max_memory << 20
        if hard != resource.RLIM_INFINITY:
            limit = min(limit, hard)
        resource.setrlimit(resource.RLIMIT_AS, (limit, hard))
