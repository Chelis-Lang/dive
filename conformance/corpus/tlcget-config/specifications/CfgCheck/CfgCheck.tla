---- MODULE CfgCheck ----
\* TLCGet("config") under model checking (AbstractChecker.createConfig).
\* The worker count, install location and polynomial are not asserted.
EXTENDS Naturals, TLC
C == TLCGet("config")
ASSUME DOMAIN C = {"mode", "deadlock", "seed", "fingerprint", "worker", "install"}
ASSUME C.mode = "bfs"
ASSUME C.deadlock = TRUE /\ C.seed = "0"
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Inv == x \in 0..2
====
