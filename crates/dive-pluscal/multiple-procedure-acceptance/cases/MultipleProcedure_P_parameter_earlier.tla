---- MODULE MultipleProcedure_P_parameter_earlier ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0,b=a+1)

begin
F:x:=x+a+b; R:return;
end procedure;
process One = 1

begin
L:call First(1,2); E:skip;
end process;
process Two = 2

begin
M:call First(2,3); N:skip;
end process;
end algorithm; *)
====
