---- MODULE ProcedureCrc_P_unknown_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P:x:=a; R:return;
end procedure;
begin
L:call Missing(2); E:skip;
end algorithm; *)
====
