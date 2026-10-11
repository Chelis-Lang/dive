---- MODULE Procedure_P_return_same_label ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: x:=a; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====
