---- MODULE ProcessHeader_P_member_recursive_base ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call First(a-1); goto R; else x:=a; goto R; end if; R:return;
end procedure;
process Worker \in {1,2}
begin
L:call First(2); E:skip;
end process;

end algorithm; *)
====
