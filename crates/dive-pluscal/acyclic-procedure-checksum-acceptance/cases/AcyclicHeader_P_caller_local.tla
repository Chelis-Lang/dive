---- MODULE AcyclicHeader_P_caller_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
variables a=0;
begin O0:a:=p+1; O1:call Inner(a); O2:return; end procedure;
procedure Inner(q=0)
begin I1:x:=q; I2:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
====
