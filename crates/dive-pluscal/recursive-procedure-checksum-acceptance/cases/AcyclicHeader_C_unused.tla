---- MODULE AcyclicHeader_C_unused ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer(p=0)
{ O1:call Inner(p+1); O2:x:=p; O3:return; };
procedure Inner(q=0)
{ I1:x:=q; I2:return; };
{ E:skip; }
} *)
====
