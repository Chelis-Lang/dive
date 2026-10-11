---- MODULE Macro_C_newline_arg ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := a; }; { L: Put(x,
1 + 2); E: skip; } } *)
====
