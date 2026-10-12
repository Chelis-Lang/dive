---- MODULE C_owners_next ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; fair+ process(P=1){L:-g:=1; M:+g:=2; E:skip;} fair process(Q=2){N:+g:=2; R:-g:=1; F:skip;}} *)
====
