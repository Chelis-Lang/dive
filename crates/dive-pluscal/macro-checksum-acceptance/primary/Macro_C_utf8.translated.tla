---- MODULE Macro_C_utf8 ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { v := <<"λ",a>>; }; { L: Put(x, 1); E: skip; } } *)
====
