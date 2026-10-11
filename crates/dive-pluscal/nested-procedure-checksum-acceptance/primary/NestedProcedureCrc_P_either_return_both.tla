---- MODULE NestedProcedureCrc_P_either_return_both ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:either return; or return; end either; end procedure; begin L:call Put(2); E:skip; end algorithm; *)
====
