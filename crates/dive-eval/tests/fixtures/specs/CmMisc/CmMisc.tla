---- MODULE CmMisc ----
EXTENDS Integers, Sequences, FiniteSets, TLC, DyadicRationals, IOUtils
ASSUME PrintT(Half([num |-> 2, den |-> 4]))
ASSUME PrintT(Half([num |-> 3, den |-> 8]))
ASSUME PrintT(Half([num |-> 0, den |-> 4]))
ASSUME PrintT(Half([num |-> -6, den |-> 4]))
ASSUME PrintT(Half([num |-> 1, den |-> 2]))
ASSUME PrintT(Add([num |-> 1, den |-> 2], [num |-> 1, den |-> 4]))
ASSUME PrintT(Add([num |-> 3, den |-> 4], [num |-> 1, den |-> 4]))
ASSUME PrintT(atoi("42"))
ASSUME PrintT(atoi("-7"))
ASSUME PrintT(atoi("+3"))
ASSUME PrintT(atoi("2147483647"))
ASSUME PrintT(atoi("007"))
====
