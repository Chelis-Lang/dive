---- MODULE NestedProcedureCrc_P_call_after_assignment ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:if x=0 then x:=1; call Put(2); goto E; else skip; end if; E:skip; end algorithm; *)
====
