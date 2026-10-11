---- MODULE AcyclicHeader_P_local_from_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:call Inner(p+1); O2:x:=p; O3:return; end procedure;
procedure Inner(q=0)
variables b=q+1;
begin I1:x:=b; I2:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
====
