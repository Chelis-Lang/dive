---- MODULE ProcessProcedure_P_main_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;

process Worker = 1

begin
L:call First(2); return;
end process;
end algorithm; *)
====
