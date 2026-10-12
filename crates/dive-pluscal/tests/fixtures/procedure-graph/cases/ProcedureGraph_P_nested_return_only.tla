---- MODULE ProcedureGraph_P_nested_return_only ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); R:return;
end procedure;
procedure Second(b=1)

begin
S:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
