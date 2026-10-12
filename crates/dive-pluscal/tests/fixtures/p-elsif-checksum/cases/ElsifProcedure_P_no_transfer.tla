---- MODULE ElsifProcedure_P_no_transfer ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0) begin P:x:=a+z; R:return; end procedure; begin L:if x=0 then skip; elsif x=1 then skip; else skip; end if; E:skip; end algorithm; *)
====
