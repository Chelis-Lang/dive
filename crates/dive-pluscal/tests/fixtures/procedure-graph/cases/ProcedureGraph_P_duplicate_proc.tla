---- MODULE ProcedureGraph_P_duplicate_proc ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F: x:=a; R: return;
end procedure;
procedure First(b=0)

begin
S:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
