---- MODULE AcyclicHeader_C_chain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer(p=0)
{ O1:call Inner(p+1); O2:x:=p; O3:return; };
procedure Inner(q=0)
{ I1:call Leaf(q+1); I2:return; };
procedure Leaf(r=0)
{ J1:x:=r; J2:return; };
{ L:call Outer(1); E:skip; }
} *)
====
