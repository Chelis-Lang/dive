---- MODULE NestedProcedureChecksum_P ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:with q=2 do if x=0 then call Put(q); goto E; else skip; end if; end with; E:skip; end algorithm; *)
====
