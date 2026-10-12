---- MODULE MacroExtra_C_record_field ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := [v |-> a, a |-> a].a; }; { L: Put(x, 1); E: skip; } } *)
====
