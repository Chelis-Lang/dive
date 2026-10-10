---- MODULE Recursive_P_unused_invalid ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call First(a-1); else x:=a; end if; R:return;
end procedure;
procedure Second(b=0)

begin
S:x:=Missing; T:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
