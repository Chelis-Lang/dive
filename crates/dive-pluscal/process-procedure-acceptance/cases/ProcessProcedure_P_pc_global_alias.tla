---- MODULE ProcessProcedure_P_pc_global_alias ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0,pc=0;
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
