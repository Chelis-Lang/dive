---- MODULE C_fair_case_lines ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:x:=CASE y=0 -> 1
        [] OTHER -> 2; E:skip; } } *)
====
