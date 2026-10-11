---- MODULE ProcedureCrc_P_return_only ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P:return;
end procedure;
begin
L:call Put(2); E:skip;
end algorithm; *)
====
