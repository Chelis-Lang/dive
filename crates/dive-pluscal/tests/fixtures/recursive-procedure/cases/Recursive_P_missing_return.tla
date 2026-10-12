---- MODULE Recursive_P_missing_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call First(a-1); else x:=a; end if;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
