---- MODULE ProcedureCrc_P_default_param ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a)

begin
P:x:=a; R:return;
end procedure;
begin
L:call Put(2); E:skip;
end algorithm; *)
====
