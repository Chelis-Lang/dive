---- MODULE RvMemUnnorm ----
EXTENDS Integers, TLC
ASSUME PrintT(<<1>> \in {[qfa |-> 1, qfb |-> 1], [qfa |-> 2, qfb |-> 2]})
====
