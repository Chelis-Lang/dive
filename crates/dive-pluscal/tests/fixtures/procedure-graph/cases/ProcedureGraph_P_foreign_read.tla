---- MODULE ProcedureGraph_P_foreign_read ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); R:x:=d; return;
end procedure;
procedure Second(b=1)
variable d=4;
begin
S:x:=b; T:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
