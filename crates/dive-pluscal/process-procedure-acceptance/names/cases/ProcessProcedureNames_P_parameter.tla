---- MODULE ProcessProcedureNames_P_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(ProcSet=0)

begin
F:x:=ProcSet; R:return;
end procedure;

process Worker = 1

begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====
