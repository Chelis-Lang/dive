---- MODULE NestedProcedureCrc_P_while_direct_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:while a=0 do return; end while; R:return; end procedure; begin L:call Put(2); E:skip; end algorithm; *)
====
