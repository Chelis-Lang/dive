---- MODULE AcyclicHeader_C_elsif ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer(p=0)
{ O1:if (p=0) { x:=0; } else { if (p=1) { call Inner(p); } else { x:=3; }; }; O2:return; };
procedure Inner(q=0)
{ I1:x:=q; I2:return; };
{ L:call Outer(1); E:skip; }
} *)
====
