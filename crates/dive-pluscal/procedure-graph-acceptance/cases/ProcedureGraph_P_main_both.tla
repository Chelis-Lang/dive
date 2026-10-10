---- MODULE ProcedureGraph_P_main_both ----
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
L:call First(2); E:call Second(3); Z:skip;
end algorithm; *)
====
