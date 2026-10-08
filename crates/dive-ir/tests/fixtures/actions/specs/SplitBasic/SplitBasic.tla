---------------------------- MODULE SplitBasic ----------------------------
(* Disjunctions split; a bounded \E over a constant set splits per
   binding, first variable fastest; a user operator with constant arguments
   is inlined and names the actions inside it, with its parameters in the
   label; one with a state-level argument is one action; labels are
   transparent; a conjunction is one action. *)
EXTENDS Naturals
VARIABLE x
A == x' = 1
B(i) == x' = i \/ x' = i + 10
C(i, j) == x' = i + j
D(v) == x' = v
Next ==
  \/ A
  \/ \E i \in {2, 1} : B(i)
  \/ \E i, j \in {1, 2} : C(i, j)
  \/ D(x)
  \/ lab :: (x' = 7 \/ x' = 8)
  \/ (x' = 9 /\ x = 0)
Init == x = 0
=============================================================================
