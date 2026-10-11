---- MODULE ProcedureCrc_P_zero_params_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put()
variable t=1;
begin
P:x:=t; R:return;
end procedure;
begin
L:call Put(); E:skip;
end algorithm; *)
====
