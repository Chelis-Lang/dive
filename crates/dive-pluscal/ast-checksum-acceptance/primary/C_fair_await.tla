---- MODULE C_fair_await ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A { variables x=0; { L: await x=0; x:=1; } } *)
====
