---- MODULE Procedure_P_operator_names ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(Next=0)

begin
P: x:=Next; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====
