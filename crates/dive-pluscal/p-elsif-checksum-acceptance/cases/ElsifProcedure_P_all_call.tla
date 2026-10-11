---- MODULE ElsifProcedure_P_all_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0) begin P:x:=a+z; R:return; end procedure; begin L:if x=0 then call Put(1,2); goto E; elsif x=1 then call Put(1,2); goto E; else call Put(1,2); goto E; end if; E:skip; end algorithm; *)
====
