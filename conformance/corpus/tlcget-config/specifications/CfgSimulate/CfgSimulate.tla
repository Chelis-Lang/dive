---- MODULE CfgSimulate ----
\* TLCGet("config") under -simulate num=3 (Simulator.createConfig): no
\* fingerprint, and traces, depth, aril and sched. The worker count and
\* install location are not asserted.
EXTENDS Naturals, TLC
C == TLCGet("config")
ASSUME DOMAIN C = {"mode", "deadlock", "seed", "worker", "install",
                   "traces", "depth", "aril", "sched"}
ASSUME C.mode = "simulate"
ASSUME C.traces = 3 /\ C.depth = 100 /\ C.aril = "0" /\ C.sched = "random"
ASSUME C.deadlock = FALSE /\ C.seed = "0"
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Inv == x \in 0..2
====
