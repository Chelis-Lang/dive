---- MODULE MultipleProcedure_P_generated_procset_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(ProcSet=0)

begin
F:x:=ProcSet; R:return;
end procedure;
process One = 1

begin
L:call First(1); E:skip;
end process;
process Two = 2

begin
M:call First(2); N:skip;
end process;
end algorithm; *)
====
