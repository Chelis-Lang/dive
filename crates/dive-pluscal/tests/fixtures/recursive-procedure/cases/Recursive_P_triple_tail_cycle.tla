---- MODULE Recursive_P_triple_tail_cycle ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call Second(a-1); return; else x:=a; end if; R:return;
end procedure;
procedure Second(b=0)

begin
S:if b>0 then call Third(b-1); return; else x:=b; end if; T:return;
end procedure;
procedure Third(c=0)

begin
U:if c>0 then call First(c-1); return; else x:=c; end if; V:return;
end procedure;
begin
L:call First(4); E:skip;
end algorithm; *)
====
