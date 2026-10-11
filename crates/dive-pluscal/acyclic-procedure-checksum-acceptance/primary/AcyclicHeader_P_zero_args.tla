---- MODULE AcyclicHeader_P_zero_args ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer()
begin O1:call Inner(); O2:return; end procedure;
procedure Inner()
begin I1:x:=x+1; I2:return; end procedure;
begin L:call Outer(); E:skip; end algorithm; *)
====
