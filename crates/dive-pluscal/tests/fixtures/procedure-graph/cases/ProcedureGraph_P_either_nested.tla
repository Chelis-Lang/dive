---- MODULE ProcedureGraph_P_either_nested ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:either call Second(a); or x:=1; end either; R:return;
end procedure;
procedure Second(b=1)

begin
S: x:=b; T: return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
