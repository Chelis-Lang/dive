# Uncredited integration tooling failure

The first archive helper received an example receipt whose `source_sha256` is a string, not a map. It stopped before producing a record. The partial copied matrices and log remain uncredited here. A separate complete `material-runtime-source-authority` archive verifies all 382 runtime inputs from producer `9b098e172d02abcf2dd3126faabf719ad5d1940a`. The first integration source-control run also stopped because the earlier feature matrix did not include every native producer input; no native result or assertion was changed.
