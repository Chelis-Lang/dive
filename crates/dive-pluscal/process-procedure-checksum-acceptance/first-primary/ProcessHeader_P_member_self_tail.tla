---- MODULE ProcessHeader_P_member_self_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call First(a-1); return; else x:=a; goto R; end if; R:return;
end procedure;
process Worker \in {1,2}
begin
L:call First(2); E:skip;
end process;

end algorithm; *)
====
