---- MODULE Macro_C_simultaneous ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(a, b) { x := <<a,b>>; }; { L: Put(b, 3); E: skip; } } *)
====
