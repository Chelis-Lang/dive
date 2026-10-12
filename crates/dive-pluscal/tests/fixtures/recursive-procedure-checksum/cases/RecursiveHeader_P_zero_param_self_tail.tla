---- MODULE RecursiveHeader_P_zero_param_self_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Recur() begin R1:call Recur(); return; end procedure;
begin L:call Recur(); E:skip; end algorithm; *)
====
