---- MODULE NestedProcedureCrc_P_nested_with_if_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:with q=a do if q=0 then return; else x:=q; end if; end with; R:return; end procedure; begin L:call Put(2); E:skip; end algorithm; *)
====
