---- MODULE NestedProcedureCrc_P_with_call_implicit_next ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:with q=2 do call Put(q); end with; E:skip; end algorithm; *)
====
