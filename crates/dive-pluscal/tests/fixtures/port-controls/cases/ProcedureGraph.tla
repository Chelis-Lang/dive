---- MODULE ProcedureGraph ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; procedure Outer(a=0) variable u=1; begin O: call Inner(a+u); R: return; end procedure; procedure Inner(b=0) variable v=2; begin I: x:=b+v; S: return; end procedure; begin L: call Outer(3); E: skip; end algorithm
*)
====
