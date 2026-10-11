---- MODULE C_fair_missing_declared ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P \in {1,2}){g:=1; g:=2;}} *)
====
