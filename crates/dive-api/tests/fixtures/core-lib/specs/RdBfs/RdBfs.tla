---- MODULE RdBfs ----
\* TLC!RandomElement on the core (crates/dive-core/src/stdlib/tlc.rs): the
\* initial states draw from the start-up stream, each state's successors
\* from the stream TLC seeds with the state's fingerprint, so the state
\* space depends on every draw.
EXTENDS Naturals, TLC
VARIABLE x
Init == x \in {RandomElement(0..60), RandomElement(0..60)}
Next == \E i \in 1..2 :
          /\ x' = (x * 3 + RandomElement(1..9) * i) % 61
          /\ PrintT(<<"e", x, i, x'>>)
====
