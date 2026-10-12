---- MODULE ProcedureGraph_P_order_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Second(b=1)

begin
S: x:=b; T: return;
end procedure;
procedure First(a=0)

begin
F: x:=a; R: return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
