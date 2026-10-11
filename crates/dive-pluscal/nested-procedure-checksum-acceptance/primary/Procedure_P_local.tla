---- MODULE Procedure_P_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)
variable b=1;
begin
P: x:=a+b; R: return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====
