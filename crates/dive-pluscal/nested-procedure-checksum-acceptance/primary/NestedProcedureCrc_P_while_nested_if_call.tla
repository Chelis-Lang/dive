---- MODULE NestedProcedureCrc_P_while_nested_if_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:while x=0 do if x=0 then call Put(2); goto B; else skip; end if; B:skip; end while; E:skip; end algorithm; *)
====
