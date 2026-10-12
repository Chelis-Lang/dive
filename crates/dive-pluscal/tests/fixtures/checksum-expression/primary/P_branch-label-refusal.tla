---- MODULE P_branch_label_refusal ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:if x=0 then B:x:=1; else skip; end if; E:skip; end algorithm; *)
====
