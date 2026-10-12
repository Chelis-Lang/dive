---- MODULE ProcedureGraph_P_parameter_order ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0,c=a+1)

begin
F:call Second(a,c); R:return;
end procedure;
procedure Second(b=1,d=b+1)

begin
S:x:=b+d; T:return;
end procedure;
begin
L:call First(2,3); E:skip;
end algorithm; *)
====
