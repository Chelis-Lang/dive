---- MODULE ProcedureGraph_P_unused_invalid ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F: x:=a; R: return;
end procedure;
procedure Second(b=1)

begin
S:x:=Missing; T:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
