---- MODULE Macro_C_empty ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put() {  }; { L: Put(); E: skip; } } *)
====
