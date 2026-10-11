---- MODULE AcyclicHeader_C_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Inner(q=0)
{ I1:x:=q; I2:return; };
procedure Outer(p=0)
{ O1:call Inner(p+1); O2:x:=p; O3:return; };
{ L:call Outer(1); E:skip; }
} *)
====
