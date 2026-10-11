---- MODULE ProcedureCrc_P_default_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)
variable t;
begin
P:t:=a; Q:x:=t; R:return;
end procedure;
begin
L:call Put(2); E:skip;
end algorithm; *)
====
