---- MODULE AcyclicHeader_C_while_true_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer(p=0)
{ O1:while (TRUE) { skip; }; call Inner(p); O2:return; };
procedure Inner(q=0)
{ I1:x:=q; I2:return; };
{ L:call Outer(1); E:skip; }
} *)
====
