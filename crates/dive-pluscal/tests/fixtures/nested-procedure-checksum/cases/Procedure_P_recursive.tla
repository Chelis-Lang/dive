---- MODULE Procedure_P_recursive ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: call Put(a); R: return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====
