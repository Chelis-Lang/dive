---- MODULE ProcedureGraph_P_label_collision ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F: x:=a; R: return;
end procedure;
procedure Second(b=1)

begin
F:x:=b; T:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
