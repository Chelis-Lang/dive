---- MODULE Procedure_P_call_missing_follow_label ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: return;
end procedure;
begin
L: call Put(2); x:=4; E: skip;
end algorithm; *)
====
