---- MODULE ElsifProcedure_C_call_1 ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {variable x=0; procedure Put(z=0,a=0) {P:x:=a+z; R:return;}; {L:if (x=0) {skip;} else if (x=1) {call Put(1,2); goto E;} else {skip;}; E:skip;}} *)
====
