---- MODULE ElsifProcedure_C_nested_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {variable x=0; procedure Put(z=0,a=0) {P:if (x=0) {skip;} else if (x=1) {if (x=0) {return;} else if (x=1) {skip;} else {return;};} else {skip;}; R:return;}; {L:call Put(1,2); goto E; E:skip;}} *)
====
