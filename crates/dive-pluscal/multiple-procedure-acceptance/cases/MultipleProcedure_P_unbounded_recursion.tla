---- MODULE MultipleProcedure_P_unbounded_recursion ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call First(a+1); R:return;
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
