---- MODULE ProcedureGraph_P_default_decls ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a)
variable c;
begin
F:call Second(a); R:return;
end procedure;
procedure Second(b=1)
variable d;
begin
S: x:=b; T: return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
