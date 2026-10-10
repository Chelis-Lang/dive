---- MODULE Inter_C_either ----
EXTENDS Naturals,TLC
(* --algorithm A { variables x=0,y=0; { either { x:=1; } or { y:=1; }; x:=2; } } *)
====
