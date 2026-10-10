---- MODULE Recursive_P_self_twice ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call First(a-1); else skip; end if; R:if a>0 then call First(a-1); else skip; end if; Z:x:=x+1; return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
