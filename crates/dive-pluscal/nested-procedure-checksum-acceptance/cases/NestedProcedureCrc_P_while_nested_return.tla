---- MODULE NestedProcedureCrc_P_while_nested_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:while a=0 do if a=0 then return; else skip; end if; end while; R:return; end procedure; begin L:call Put(2); E:skip; end algorithm; *)
====
