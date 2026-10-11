---- MODULE MultipleProcedure_P_nested_calls ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); R:x:=x+a; return;
end procedure;
procedure Second(b=0)

begin
S:x:=x+b; T:return;
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
