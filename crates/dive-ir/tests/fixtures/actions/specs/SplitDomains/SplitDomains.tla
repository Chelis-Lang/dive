--------------------------- MODULE SplitDomains ---------------------------
(* A \E whose domain is empty, or cannot be evaluated in the empty state
   because it reads a variable, is one action. Tuple patterns and several
   bound groups split per binding. *)
EXTENDS Naturals
CONSTANT N
VARIABLE x
A(i) == x' = i
Next ==
  \/ \E i \in {} : A(i)
  \/ \E i \in {x} : A(i)
  \/ \E <<i, j>> \in {<<1, 2>>, <<3, 4>>} : A(i + j)
  \/ \E i \in 1..N, j \in {5} : A(i * j)
Init == x = 0
=============================================================================
