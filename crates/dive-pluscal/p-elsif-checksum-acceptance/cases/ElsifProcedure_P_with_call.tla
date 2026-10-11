---- MODULE ElsifProcedure_P_with_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0) begin P:x:=a+z; R:return; end procedure; begin L:with q=2 do if x=0 then call Put(1,2); goto E; elsif x=1 then skip; else call Put(1,2); goto E; end if; end with; E:skip; end algorithm; *)
====
