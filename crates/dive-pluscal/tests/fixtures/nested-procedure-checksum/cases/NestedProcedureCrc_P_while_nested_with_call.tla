---- MODULE NestedProcedureCrc_P_while_nested_with_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:while x=0 do with q=2 do call Put(q); goto B; end with; B:skip; end while; E:skip; end algorithm; *)
====
