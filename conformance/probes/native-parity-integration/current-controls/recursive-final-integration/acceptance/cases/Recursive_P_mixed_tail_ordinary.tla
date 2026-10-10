---- MODULE Recursive_P_mixed_tail_ordinary ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call Second(a-1); return; else x:=a; end if; R:return;
end procedure;
procedure Second(b=0)

begin
S:if b>0 then call First(b-1); else x:=b; end if; T:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
