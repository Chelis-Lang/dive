---- MODULE C_nonliteral_domain ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P \in Nat){L:g:=g+1; E:skip;}} *)
====
