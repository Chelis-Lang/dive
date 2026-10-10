---- MODULE ProcedureGraph_P_auto_labels ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
call Second(a); return;
end procedure;
procedure Second(b=1)

begin
x:=b; return;
end procedure;
begin
call First(2); skip;
end algorithm; *)
====
