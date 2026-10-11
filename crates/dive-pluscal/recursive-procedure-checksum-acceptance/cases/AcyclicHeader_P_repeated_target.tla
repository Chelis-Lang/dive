---- MODULE AcyclicHeader_P_repeated_target ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:call Inner(p); O2:call Inner(p+1); O3:return; end procedure;
procedure Inner(q=0)
begin I1:x:=q; I2:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
====
