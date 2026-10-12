---- MODULE Procedure_P_default_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)
variable b;
begin
P: x:=a; R: return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====
