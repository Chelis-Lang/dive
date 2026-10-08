---- MODULE RndTraceAssume ----
\* TLC surprise. An ASSUME draws from the start-up stream before Init. To print
\* the counterexample TLC re-evaluates Init with a reset generator, which no
\* longer yields the initial state it found, so the trace cannot be rebuilt.
EXTENDS Naturals, TLC
ASSUME PrintT(<<"assume", RandomElement(1..1000000)>>)
VARIABLES x, y
Init == /\ x \in {RandomElement(1..1000000), RandomElement(1..1000000)}
        /\ y = 0
Next == /\ y' = y + 1
        /\ x' = x
Inv == y < 3
====
