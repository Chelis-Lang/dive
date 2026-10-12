---- MODULE MultipleProcedure_P_two_string_constants ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One = "z"

begin
L:call First(1); E:skip;
end process;
process Two = "a"

begin
M:call First(2); N:skip;
end process;
end algorithm; *)
====
