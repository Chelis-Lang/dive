---- MODULE EnvironmentOrder ----
EXTENDS IOUtils, TLC, Sequences
Environment == IOEnv
VARIABLE x
Init == x = CHOOSE name \in DOMAIN Environment : TRUE
Next == UNCHANGED x
Inv == SubSeq(x, 6, 6) = "C"
====
