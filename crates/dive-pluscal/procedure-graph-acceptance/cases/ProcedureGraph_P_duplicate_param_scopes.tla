---- MODULE ProcedureGraph_P_duplicate_param_scopes ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F: x:=a; R: return;
end procedure;
procedure Second(a=1)

begin
S:x:=a; T:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
