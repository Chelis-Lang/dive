---- MODULE FairProcedure_P_constants_reach_false_branch ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if FALSE then call Second(a); else x:=x+a; end if; R:return;
end procedure;
procedure Second(b=0)

begin
S:x:=x+b; T:return;
end procedure;
fair process One = 1

begin
L:call First(1); E:skip;
end process;
fair + process Two = 2

begin
M:call First(2); N:skip;
end process;
end algorithm; *)
====
