---- MODULE AcyclicHeader_C_local_from_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer(p=0)
{ O1:call Inner(p+1); O2:x:=p; O3:return; };
procedure Inner(q=0)
variables b=q+1;
{ I1:x:=b; I2:return; };
{ L:call Outer(1); E:skip; }
} *)
====
