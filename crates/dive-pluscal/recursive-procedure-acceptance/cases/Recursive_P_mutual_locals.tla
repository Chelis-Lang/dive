---- MODULE Recursive_P_mutual_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=a+1;
begin
F:if a>0 then call Second(a-1); else x:=u; end if; R:x:=x+u; return;
end procedure;
procedure Second(b=0)
variable v=b+2;
begin
S:if b>0 then call First(b-1); else x:=v; end if; T:x:=x+v; return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
====
