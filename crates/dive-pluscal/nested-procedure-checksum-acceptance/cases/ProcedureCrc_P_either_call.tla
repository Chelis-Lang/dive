---- MODULE ProcedureCrc_P_either_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P:x:=a; R:return;
end procedure;
begin
L:either call Put(2); goto E; or call Put(3); goto E; end either; E:skip;
end algorithm; *)
====
