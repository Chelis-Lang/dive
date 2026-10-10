---- MODULE ProcedureGraph_P_return_transfer_assign ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:a:=a+1; return;
end procedure;
procedure Second(b=1)

begin
S: x:=b; T: return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
