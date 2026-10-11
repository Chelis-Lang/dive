---- MODULE C_separator_only_semi ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:with(;){x:=a;}; E:skip; } } *)
====
