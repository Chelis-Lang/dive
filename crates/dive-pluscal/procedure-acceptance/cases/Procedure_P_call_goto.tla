---- MODULE Procedure_P_call_goto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: x:=a; return;
end procedure;
begin
L: call Put(2); goto E; E: skip;
end algorithm; *)
====
