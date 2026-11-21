---- MODULE CfgGenerate ----
\* TLCGet("config") under -generate num=2: mode "generate", otherwise the
\* simulation record. The worker count and install location are not asserted.
EXTENDS Naturals, TLC
C == TLCGet("config")
ASSUME DOMAIN C = {"mode", "deadlock", "seed", "worker", "install",
                   "traces", "depth", "aril", "sched"}
ASSUME C.mode = "generate"
ASSUME C.traces = 2 /\ C.depth = 100 /\ C.aril = "0" /\ C.sched = "random"
ASSUME C.deadlock = TRUE /\ C.seed = "0"
VARIABLE x
Init == x = 0
Next == x >= 0 /\ x' = (x + 1) % 3
Inv == x \in 0..2
====
