---- MODULE AcyclicHeader_C_target_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer(p=0)
{ O1:call Inner(p+1); O2:x:=p; O3:return; };
procedure Inner(q=0)
variables b=0;
{ I1:b:=q+1; I2:x:=b; I3:return; };
{ L:call Outer(1); E:skip; }
} *)
====
