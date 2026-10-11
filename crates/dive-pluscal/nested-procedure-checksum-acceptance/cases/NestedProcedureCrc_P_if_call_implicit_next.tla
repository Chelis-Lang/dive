---- MODULE NestedProcedureCrc_P_if_call_implicit_next ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:if x=0 then call Put(2); else call Put(3); end if; E:skip; end algorithm; *)
====
