---- MODULE NestedProcedureCrc_P_if_with_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:if x=0 then with q=2 do call Put(q); goto E; end with; else skip; end if; E:skip; end algorithm; *)
====
