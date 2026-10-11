---- MODULE AcyclicHeader_P_either ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:either call Inner(p); or x:=0; end either; O2:return; end procedure;
procedure Inner(q=0)
begin I1:x:=q; I2:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
====
