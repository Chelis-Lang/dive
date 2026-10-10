---- MODULE MacroExtra_C_function_capture ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := [a \in {1,2} |-> a]; }; { L: Put(x, 1); E: skip; } } *)
====
