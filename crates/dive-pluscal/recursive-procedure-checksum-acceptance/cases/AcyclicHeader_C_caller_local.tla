---- MODULE AcyclicHeader_C_caller_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer(p=0)
variables a=0;
{ O0:a:=p+1; O1:call Inner(a); O2:return; };
procedure Inner(q=0)
{ I1:x:=q; I2:return; };
{ L:call Outer(1); E:skip; }
} *)
====
