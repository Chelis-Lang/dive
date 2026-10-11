---- MODULE AcyclicHeader_P_call_goto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:call Inner(p); goto O3; O2:x:=99; O3:return; end procedure;
procedure Inner(q=0)
begin I1:x:=q; I2:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
====
