---- MODULE FairProcedure_P_constants_reach_mutual ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call Second(a-1); else x:=x+1; end if; R:return;
end procedure;
procedure Second(b=0)

begin
S:if b>0 then call First(b-1); else x:=x+1; end if; T:return;
end procedure;
fair process One = 1

begin
L:call First(1); E:skip;
end process;
fair + process Two = 2

begin
M:call Second(2); N:skip;
end process;
end algorithm; *)
====
