---- MODULE NestedProcedureCrc_P_either_if_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:either if x=0 then call Put(2); goto E; else skip; end if; or skip; end either; E:skip; end algorithm; *)
====
