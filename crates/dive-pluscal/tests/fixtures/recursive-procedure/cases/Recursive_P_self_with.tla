---- MODULE Recursive_P_self_with ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:with n=a-1 do if a>0 then call First(n); else x:=x+1; end if; end with; R:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
