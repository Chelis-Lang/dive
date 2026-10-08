----------------------------- MODULE SplitLet -----------------------------
(* LET is transparent, and its definitions are not bound while TLC
   splits: a zero-arity LET definition used as a disjunct is found by name
   and inlined, naming its actions; a LET operator with constant arguments
   is inlined too. *)
EXTENDS Naturals
VARIABLE x
Next ==
  LET A == x' = 1 \/ x' = 2
      B(i) == x' = i
  IN \/ A
     \/ \E i \in {5, 6} : B(i)
     \/ x' = 3
Init == x = 0
=============================================================================
