---- MODULE ProcedureGraph_P_call_return_main ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F: x:=a; R: return;
end procedure;
procedure Second(b=1)

begin
S: x:=b; T: return;
end procedure;
begin
L:call First(2); return;
end algorithm; *)
====
