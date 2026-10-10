---- MODULE ProcedureGraph_P_call_return_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable c=a+1;
begin
F:call Second(c); return;
end procedure;
procedure Second(b=1)
variable d=b+2;
begin
S:x:=d; T:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
