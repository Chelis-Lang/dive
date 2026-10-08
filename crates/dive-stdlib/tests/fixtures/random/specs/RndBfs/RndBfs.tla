---- MODULE RndBfs ----
\* A random graph: the initial states come from the start-up stream, the edges
\* from each state's own stream. The printed lines follow TLC's breadth-first
\* order, so the state space itself depends on every draw.
EXTENDS Naturals, TLC
VARIABLE x
Init == x \in {RandomElement(0..60), RandomElement(0..60)}
Next == \E i \in 1..2 :
          LET d == RandomElement(1..9) IN
          /\ PrintT(<<"e", x, i, d>>)
          /\ x' = (x * 3 + d * i) % 61
====
