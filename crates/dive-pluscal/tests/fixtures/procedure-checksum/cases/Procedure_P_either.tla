---- MODULE Procedure_P_either ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: either x:=a; or x:=1; end either; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
====
