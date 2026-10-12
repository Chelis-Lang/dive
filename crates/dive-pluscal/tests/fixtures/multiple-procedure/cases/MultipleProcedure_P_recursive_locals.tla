---- MODULE MultipleProcedure_P_recursive_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=a;
begin
F:if a>0 then call First(a-1); else x:=x+u; end if; R:x:=x+u; return;
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
