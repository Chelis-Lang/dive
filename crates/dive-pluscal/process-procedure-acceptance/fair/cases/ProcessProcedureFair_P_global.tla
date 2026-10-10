---- MODULE ProcessProcedureFair_P_global ----
EXTENDS Integers, Sequences, TLC
(* --fair algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;

process Worker = 1

begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====
