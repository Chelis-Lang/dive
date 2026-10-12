---- MODULE ElsifProcedure_C_either_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {variable x=0; procedure Put(z=0,a=0) {P:x:=a+z; R:return;}; {L:either {if (x=0) {call Put(1,2); goto E;} else if (x=1) {skip;} else {call Put(1,2); goto E;};} or {skip;}; E:skip;}} *)
====
