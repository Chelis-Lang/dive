The outer preflight wrapper omitted an explicit Cargo target. Its inventory
checker was stopped (exit-15) after the first34 checks passed. Only its owned
process descendants and verified inactive worktree-local generated target were
removed; active private targets and evidence were preserved. Subsequent outer
orchestration requires exact target/jobs/private-cache environment before any
child command and propagates it, including Cargo launched by Python checks.
No code assertions, inventory selection or test timeouts were changed.
