---- MODULE NestedProcedureCrc_P_if_call_empty_else ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:if x=0 then call Put(2); goto E; else end if; E:skip; end algorithm; *)
====
