---- MODULE ProcedureCrc_P_zero_slots ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put()

begin
P:x:=2; R:return;
end procedure;
begin
L:call Put(); E:skip;
end algorithm; *)
====
