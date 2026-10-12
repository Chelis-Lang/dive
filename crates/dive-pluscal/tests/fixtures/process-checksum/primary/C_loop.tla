---- MODULE C_loop ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = 1){L:g:=1; goto L;}} *)
====
