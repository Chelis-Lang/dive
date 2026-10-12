---- MODULE NestedProcedureCrc_P_either_call_mixed ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:either call Put(2); goto E; or skip; end either; E:skip; end algorithm; *)
====
