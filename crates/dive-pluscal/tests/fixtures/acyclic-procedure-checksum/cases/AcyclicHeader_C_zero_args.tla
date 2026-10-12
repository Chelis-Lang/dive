---- MODULE AcyclicHeader_C_zero_args ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variables x=0;
procedure Outer()
{ O1:call Inner(); O2:return; };
procedure Inner()
{ I1:x:=x+1; I2:return; };
{ L:call Outer(); E:skip; }
} *)
====
