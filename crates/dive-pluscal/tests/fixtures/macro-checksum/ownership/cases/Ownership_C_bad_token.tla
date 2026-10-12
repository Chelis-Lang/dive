---- MODULE Ownership_C_bad_token ----
EXTENDS Integers
(* --algorithm A { variable x=0; macro M(a) { x:=a; }; { L:M(1); E:skip; } } ` *)
====
