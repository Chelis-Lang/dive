---- MODULE AcyclicHeader_P_elsif ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variables x=0;
procedure Outer(p=0)
begin O1:if p=0 then x:=0; elsif p=1 then call Inner(p); else x:=3; end if; O2:return; end procedure;
procedure Inner(q=0)
begin I1:x:=q; I2:return; end procedure;
begin L:call Outer(1); E:skip; end algorithm; *)
====
