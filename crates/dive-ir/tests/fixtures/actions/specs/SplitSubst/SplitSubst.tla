---------------------------- MODULE SplitSubst ----------------------------
(* An instance with substitutions: I!Next is a substitution that
   getActions does not split, so INext inlines it and gets one action, named
   I!Next, although Inner's Next is a disjunction. *)
EXTENDS Naturals
VARIABLE y
---- MODULE Inner ----
VARIABLE x
Next == x' = 1 \/ x' = 2
====
I == INSTANCE Inner WITH x <- y
Init == y = 0
INext == I!Next
=============================================================================
