---- MODULE ProcedureGraph_P_no_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First()

begin
F:call Second(); R:return;
end procedure;
procedure Second()

begin
S:x:=1; T:return;
end procedure;
begin
L:call First(); E:skip;
end algorithm; *)
====
