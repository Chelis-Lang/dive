---- MODULE ProcedureGraph_P_nested ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a+1); R:x:=x+a; return;
end procedure;
procedure Second(b=1)

begin
S: x:=b; T: return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
