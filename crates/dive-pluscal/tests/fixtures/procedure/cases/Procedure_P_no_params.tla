---- MODULE Procedure_P_no_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put()

begin
P: x:=1; R: return;
end procedure;
begin
L: call Put(); E: skip;
end algorithm; *)
====
