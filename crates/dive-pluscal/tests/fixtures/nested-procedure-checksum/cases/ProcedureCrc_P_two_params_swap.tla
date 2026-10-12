---- MODULE ProcedureCrc_P_two_params_swap ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0,b=1)

begin
P:x:=a+b; R:return;
end procedure;
begin
L:call Put(b,a); E:skip;
end algorithm; *)
====
