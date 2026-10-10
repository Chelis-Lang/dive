The first gate invocation omitted the explicit `--dive-bin` argument.
Prepending the release target to PATH does not select the harness implementation
executable. Both invocations refused at configuration before running models.
The corrected invocations select the exact release executable; full 21-model CM1
and 16-model ValidationTestSuite selections pass at 1/4/8 workers. Runtime
implementation and oracle evidence were unchanged. Original logs are retained.
