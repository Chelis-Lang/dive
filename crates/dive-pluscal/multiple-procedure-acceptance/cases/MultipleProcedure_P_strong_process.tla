---- MODULE MultipleProcedure_P_strong_process ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One = 1

begin
L:call First(1); E:skip;
end process;
fair + process Two = 2

begin
M:call First(2); N:skip;
end process;
end algorithm; *)
====
