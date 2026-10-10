---- MODULE Recursive_P_mutual_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Second(b=0)

begin
S:if b>0 then call First(b-1); else x:=1; end if; T:return;
end procedure;
procedure First(a=0)

begin
F:if a>0 then call Second(a-1); else x:=0; end if; R:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
