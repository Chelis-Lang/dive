---- MODULE ProcedureGraph_P_conditional_cycle ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if FALSE then call First(a); else x:=a; end if; R:return;
end procedure;
procedure Second(b=1)

begin
S: x:=b; T: return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
====
