---- MODULE NestedProcedureCrc_P_either_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:either return; or x:=a; end either; R:return; end procedure; begin L:call Put(2); E:skip; end algorithm; *)
====
