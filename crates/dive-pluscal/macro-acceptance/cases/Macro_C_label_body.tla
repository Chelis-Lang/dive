---- MODULE Macro_C_label_body ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { B: v:=a; }; { L: Put(x, 1); E: skip; } } *)
====
