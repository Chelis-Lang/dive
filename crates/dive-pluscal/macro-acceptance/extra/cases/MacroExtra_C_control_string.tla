---- MODULE MacroExtra_C_control_string ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := <<"",a>>; }; { L: Put(x, 1); E: skip; } } *)
====
