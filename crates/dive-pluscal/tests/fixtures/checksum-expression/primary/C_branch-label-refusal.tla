---- MODULE C_branch_label_refusal ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:if(x=0){B:x:=1;}else{skip;} E:skip; } } *)
====
