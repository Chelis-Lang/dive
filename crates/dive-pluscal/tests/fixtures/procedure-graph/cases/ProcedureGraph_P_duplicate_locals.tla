---- MODULE ProcedureGraph_P_duplicate_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable c=0;
begin
F: x:=a; R: return;
end procedure;
procedure Second(b=1)
variable c=1;
begin
S: x:=b; T: return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
