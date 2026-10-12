---- MODULE ProcedureGraph_P_separate_local_init ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); R:return;
end procedure;
procedure Second(b=1)
variable d=b+1,e=d+1;
begin
S:x:=d+e; T:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
