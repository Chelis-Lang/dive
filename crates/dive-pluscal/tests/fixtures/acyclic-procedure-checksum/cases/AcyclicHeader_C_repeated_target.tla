---- MODULE AcyclicHeader_C_repeated_target ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer(p=0)
{ O1:call Inner(p); O2:call Inner(p+1); O3:return; };
procedure Inner(q=0)
{ I1:x:=q; I2:return; };
{ L:call Outer(1); E:skip; }
} *)
====
