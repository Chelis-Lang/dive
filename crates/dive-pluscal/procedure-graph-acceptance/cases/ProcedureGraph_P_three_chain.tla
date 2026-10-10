---- MODULE ProcedureGraph_P_three_chain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); R:return;
end procedure;
procedure Second(b=1)

begin
S:call Third(b+1); T:x:=x+b; return;
end procedure;
procedure Third(c=0)
variable d=c+1;
begin
U:x:=d; V:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
