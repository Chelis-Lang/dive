---- MODULE C_goto ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x=0; { L: x:=1; goto L; } } *)
====
