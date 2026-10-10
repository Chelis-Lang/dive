---- MODULE Procedure_P_source_assert ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: assert a>0; x:=a; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====
