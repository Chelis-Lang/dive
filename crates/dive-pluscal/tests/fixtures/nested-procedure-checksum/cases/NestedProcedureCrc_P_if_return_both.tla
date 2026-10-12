---- MODULE NestedProcedureCrc_P_if_return_both ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:if a=0 then return; else return; end if; end procedure; begin L:call Put(2); E:skip; end algorithm; *)
====
