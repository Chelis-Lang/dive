---- MODULE ProcedureGraph_P_two_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0,c=1)

begin
F:call Second(a,c); R:return;
end procedure;
procedure Second(b=1,d=2)

begin
S:x:=b+d; T:return;
end procedure;
begin
L:call First(2,3); E:skip;
end algorithm; *)
====
