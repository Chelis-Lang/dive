---- MODULE Ownership_C_unclosed_quote ----
EXTENDS Integers
(* --algorithm A { variable x=0; macro M(a) { x:=a; }; { L:M(1); E:skip; } } "abc *)
====
