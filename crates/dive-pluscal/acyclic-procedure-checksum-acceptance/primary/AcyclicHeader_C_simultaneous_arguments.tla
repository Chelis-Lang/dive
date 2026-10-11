---- MODULE AcyclicHeader_C_simultaneous_arguments ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer(p=0)
{ O1:call Inner(p+1,p); O2:return; };
procedure Inner(q=0,r=0)
{ I1:x:=q+r; I2:return; };
{ L:call Outer(1); E:skip; }
} *)
====
