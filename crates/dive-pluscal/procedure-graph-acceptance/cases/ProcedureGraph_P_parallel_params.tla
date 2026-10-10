---- MODULE ProcedureGraph_P_parallel_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0,c=1)

begin
F:a:=c || c:=a; R:call Second(a,c); Z:return;
end procedure;
procedure Second(b=1,d=2)

begin
S:b:=d || d:=b; T:x:=b+d; return;
end procedure;
begin
L:call First(2,3); E:skip;
end algorithm; *)
====
