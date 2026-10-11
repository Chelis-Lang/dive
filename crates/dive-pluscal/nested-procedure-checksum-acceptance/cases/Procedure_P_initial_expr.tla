---- MODULE Procedure_P_initial_expr ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=4)
variable b=a+1;
begin
P: x:=b; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====
