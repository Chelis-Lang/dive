---- MODULE NestedProcedureCrc_P_while_direct_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:while x=0 do call Put(2); B:skip; end while; E:skip; end algorithm; *)
====
