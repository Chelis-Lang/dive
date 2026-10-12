---- MODULE ProcedureGraph_P_local_restore ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable c=9;
begin
F:c:=a+1; call Second(c); R:x:=c; return;
end procedure;
procedure Second(b=1)
variable d=4;
begin
S:d:=b+1; x:=d; T:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
