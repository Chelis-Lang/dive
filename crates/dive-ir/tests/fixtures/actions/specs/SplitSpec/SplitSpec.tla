----------------------------- MODULE SplitSpec -----------------------------
(* With SPECIFICATION, the next-state relation is the A of [][A]_v, with
   no name of its own: a disjunct directly in it is named "Action"; an
   operator it uses is inlined and names its actions. *)
EXTENDS Naturals
VARIABLE x
Inc == x' = x + 1
Spec == x = 0 /\ [][Inc \/ x' = 0]_x
=============================================================================
