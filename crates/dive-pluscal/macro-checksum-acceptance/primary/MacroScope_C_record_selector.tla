---- MODULE MacroScope_C_record_selector ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0,b=1; macro Put(v,a) { v:= [a |-> a].a; }; { L: Put(x,2); E: skip; } } *)
====
