---- MODULE C_fair_junction ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A { variables x=0; { L: await /\ TRUE /\ x=0; } } *)
====
