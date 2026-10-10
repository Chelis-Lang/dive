---- MODULE ProcedureGraph_P_three_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); return;
end procedure;
procedure Second(b=1)

begin
S:call Third(b); return;
end procedure;
procedure Third(c=0)

begin
U:x:=c; V:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
