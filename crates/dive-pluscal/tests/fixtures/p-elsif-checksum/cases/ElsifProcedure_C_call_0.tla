---- MODULE ElsifProcedure_C_call_0 ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {variable x=0; procedure Put(z=0,a=0) {P:x:=a+z; R:return;}; {L:if (x=0) {call Put(1,2); goto E;} else if (x=1) {skip;} else {skip;}; E:skip;}} *)
====
