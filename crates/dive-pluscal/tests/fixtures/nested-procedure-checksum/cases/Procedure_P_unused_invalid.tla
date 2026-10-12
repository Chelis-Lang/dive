---- MODULE Procedure_P_unused_invalid ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: x:=Missing; return;
end procedure;
begin
L: skip;
end algorithm; *)
====
