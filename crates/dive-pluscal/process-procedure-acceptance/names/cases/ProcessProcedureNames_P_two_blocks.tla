---- MODULE ProcessProcedureNames_P_two_blocks ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;

process Worker = 1

begin
L:call First(2); E:skip;
end process;
process Other = 2
begin
O:call First(1); Z:skip;
end process;
end algorithm; *)
====
