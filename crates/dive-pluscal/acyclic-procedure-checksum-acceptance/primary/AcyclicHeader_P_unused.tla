---- MODULE AcyclicHeader_P_unused ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:call Inner(p+1); O2:x:=p; O3:return; end procedure;
procedure Inner(q=0)
begin I1:x:=q; I2:return; end procedure;
begin E:skip; end algorithm; *)
====
