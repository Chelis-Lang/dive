---- MODULE ProcessProcedure_P_process_name_alias ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(Worker=0)

begin
F:x:=a; R:return;
end procedure;

process Worker = 1

begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====
