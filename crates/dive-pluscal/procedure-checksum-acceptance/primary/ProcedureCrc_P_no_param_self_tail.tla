---- MODULE ProcedureCrc_P_no_param_self_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put()

begin
P:call Put(); return;
end procedure;
begin
L:call Put(); E:skip;
end algorithm; *)
====
