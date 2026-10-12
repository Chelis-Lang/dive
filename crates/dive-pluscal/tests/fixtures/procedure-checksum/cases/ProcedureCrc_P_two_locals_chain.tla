---- MODULE ProcedureCrc_P_two_locals_chain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)
variables t=a+1,u=t+1;
begin
P:x:=u; R:return;
end procedure;
begin
L:call Put(2); E:skip;
end algorithm; *)
====
