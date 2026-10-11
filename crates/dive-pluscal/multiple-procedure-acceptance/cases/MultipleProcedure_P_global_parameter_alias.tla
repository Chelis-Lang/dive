---- MODULE MultipleProcedure_P_global_parameter_alias ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(x=0)

begin
F:x:=x+a; R:return;
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
