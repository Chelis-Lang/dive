---- MODULE AcyclicHeader_C_call_goto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer(p=0)
{ O1:call Inner(p); goto O3; O2:x:=99; O3:return; };
procedure Inner(q=0)
{ I1:x:=q; I2:return; };
{ L:call Outer(1); E:skip; }
} *)
====
