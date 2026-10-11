---- MODULE Procedure_P_implicit_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: x:=a;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====
