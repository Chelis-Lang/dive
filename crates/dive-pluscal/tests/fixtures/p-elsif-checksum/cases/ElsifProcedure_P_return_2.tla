---- MODULE ElsifProcedure_P_return_2 ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0) begin P:if x=0 then skip; elsif x=1 then skip; else return; end if; R:return; end procedure; begin L:call Put(1,2); goto E; E:skip; end algorithm; *)
====
