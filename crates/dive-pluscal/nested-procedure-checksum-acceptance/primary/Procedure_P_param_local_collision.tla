---- MODULE Procedure_P_param_local_collision ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)
variable a=1;
begin
P: x:=a; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====
