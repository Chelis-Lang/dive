--------------------------- MODULE SplitOverride ---------------------------
(* The configuration replaces Step with Other (Step <- Other): TLC's lookup
   finds Other, so Next's use of Step inlines Other, under Other's name. *)
EXTENDS Naturals
VARIABLE x
Step == x' = 1
Other == x' = 2 \/ x' = 3
Next == Step \/ x' = 4
Init == x = 0
=============================================================================
