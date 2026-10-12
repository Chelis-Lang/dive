---- MODULE ProcedureGraph_P_read_stack ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); R:x:=Len(stack); return;
end procedure;
procedure Second(b=1)

begin
S:x:=Len(stack); T:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
