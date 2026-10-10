---- MODULE ProcedureGraph_P_reserved_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F: x:=a; R: return;
end procedure;
procedure Second(Next=1)

begin
S:x:=Next; T:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
