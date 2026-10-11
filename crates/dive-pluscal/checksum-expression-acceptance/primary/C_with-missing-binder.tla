---- MODULE C_with_missing_binder ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:with(a=1,){skip;} E:skip; } } *)
====
