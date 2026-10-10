---- MODULE ProcedureGraph_P_nested_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable c=1;
begin
F:c:=a+1; call Second(c); R:x:=x+c; return;
end procedure;
procedure Second(b=1)
variable d=b+1;
begin
S:x:=d; T:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
