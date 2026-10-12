---- MODULE Procedure_P_global_param_collision ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(x=0)

begin
P: x:=1; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====
