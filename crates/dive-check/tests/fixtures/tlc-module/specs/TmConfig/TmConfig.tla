---- MODULE TmConfig ----
EXTENDS Naturals, TLC
VARIABLE x
c == TLCGet("config")
ASSUME PrintT(<<c.mode, c.deadlock, c.worker, c.seed, c.fingerprint>>)
ASSUME PrintT(DOMAIN c)
ASSUME PrintT(<<TLCGet("generated"), TLCGet("distinct"), TLCGet("queue"), TLCGet("diameter")>>)
Init == x = 0
Next == x < 2 /\ x' = x + 1 /\ PrintT(TLCGet("config").worker)
====
