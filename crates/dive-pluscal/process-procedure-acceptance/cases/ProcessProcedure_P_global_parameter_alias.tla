---- MODULE ProcessProcedure_P_global_parameter_alias ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(x=0)

begin
F:x:=a; R:return;
end procedure;

process Worker = 1

begin
L:call First(2); E:skip;
end process;
end algorithm; *)
====
