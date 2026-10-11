---- MODULE NestedProcedureCrc_P_if_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:if a=0 then return; else x:=a; end if; R:return; end procedure; begin L:call Put(2); E:skip; end algorithm; *)
====
