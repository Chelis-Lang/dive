---- MODULE ElsifProcedure_C_either_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {variable x=0; procedure Put(z=0,a=0) {P:either {if (x=0) {return;} else if (x=1) {skip;} else {return;};} or {skip;}; R:return;}; {L:call Put(1,2); goto E; E:skip;}} *)
====
