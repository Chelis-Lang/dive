---- MODULE ProcedureGraph_P_callee_name_collision ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F: x:=a; R: return;
end procedure;
procedure Second(First=1)

begin
S:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
