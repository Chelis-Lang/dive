---- MODULE Procedure_P_duplicate_param ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0,a=1)

begin
P: x:=a; return;
end procedure;
begin
L: call Put(2,3); E: skip;
end algorithm; *)
====
