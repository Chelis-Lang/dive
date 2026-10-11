---- MODULE NestedProcedureCrc_P_with_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:with q=a do return; end with; end procedure; begin L:call Put(2); E:skip; end algorithm; *)
====
