---- MODULE C_fair_goto ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A { variables x=0; { L: x:=1; goto L; } } *)
====
