---- MODULE NestedProcedureCrc_P_nested_arg_record ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:with q=[v |-> 2] do call Put(q.v); goto E; end with; E:skip; end algorithm; *)
====
